use axum::{
    async_trait,
    extract::FromRequestParts,
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    RequestPartsExt, TypedHeader,
};
use std::{path::Path, sync::Arc};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};

use card_deck::standard_deck::Card;
use errors::Error;
use pasetors::{
    claims::ClaimsValidationRules,
    keys::{AsymmetricKeyPair, AsymmetricSecretKey},
    version4::V4,
};
use rooms::{Action, GameState};
use serde::{Deserialize, Serialize};
use server::Server;
use tokio::sync::{mpsc, oneshot};
use tower_http::services::ServeDir;
use uuid::Uuid;

mod errors;
mod rooms;
mod server;

#[derive(Clone)]
struct ServerState {
    // ED25519 key for signing PASETO tokens
    sender: RouterServerMessageSender,
    key_pair: AsymmetricKeyPair<V4>,
}

/// Create a router for बदाम सात.
pub fn badam_sat_router<P: AsRef<Path>>(
    key_pair: AsymmetricKeyPair<V4>,
    max_rooms: usize,
    frontend_path: P,
) -> Router {
    let (sender, receiver) = mpsc::channel(100);
    Server::spawn(max_rooms, receiver);
    let sender = Arc::new(sender);
    let state = ServerState { sender, key_pair };

    let serve_dir = ServeDir::new(frontend_path);
    Router::new()
        .route("/api/create_room", post(create_room))
        .route("/api/join", post(join))
        .route("/api/play", post(play))
        .route("/api/game_state", get(game_state))
        .route("/api/my_hand", get(hand_of_player))
        .route("/api/last_move", get(last_move))
        .fallback_service(serve_dir)
        .with_state(state)
}

type RouterServerMessageSender = Arc<mpsc::Sender<RouterServerMessage>>;

enum RouterServerMessage {
    CreateRoom {
        players: usize,
        decks: usize,
        responder: oneshot::Sender<Result<Uuid, Error>>,
    },
    JoinRoom {
        room: Uuid,
        secret_key: AsymmetricSecretKey<V4>,
        responder: oneshot::Sender<Result<String, Error>>,
    },
    Play {
        action: Action,
        player: usize,
        room: Uuid,
        responder: oneshot::Sender<Result<(), Error>>,
    },
    GetHand {
        player: usize,
        room: Uuid,
        responder: oneshot::Sender<Result<Vec<Card>, Error>>,
    },
    LastMove {
        room: Uuid,
        responder: oneshot::Sender<Result<Action, Error>>,
    },
    GameState {
        room: Uuid,
        responder: oneshot::Sender<Result<GameState, Error>>,
    },
}

async fn create_room(
    State(state): State<ServerState>,
    Json(room_request): Json<NewRoomRequest>,
) -> Result<Json<RoomPayload>, Error> {
    log::info!("received create room request");
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::CreateRoom {
            players: room_request.players,
            decks: room_request.decks,
            responder,
        })
        .await?;
    receiver.await?.map(|room_id| Json(RoomPayload { room_id }))
}

async fn join(
    State(state): State<ServerState>,
    Json(payload): Json<RoomPayload>,
) -> Result<Json<JoinSuccess>, Error> {
    log::info!("received join request");
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::JoinRoom {
            room: payload.room_id,
            secret_key: state.key_pair.secret,
            responder,
        })
        .await?;
    receiver.await?.map(|token| {
        Json(JoinSuccess {
            token_type: "Bearer".into(),
            token,
        })
    })
}

async fn play(
    player: AuthenticatedPlayer,
    State(state): State<ServerState>,
    Json(action): Json<Action>,
) -> Result<StatusCode, Error> {
    log::info!("received play request from player {}", player.player_id);
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::Play {
            action,
            player: player.player_id,
            room: player.room_id,
            responder,
        })
        .await?;
    receiver.await?.map(|_| StatusCode::OK)
}

async fn game_state(
    State(state): State<ServerState>,
    Query(payload): Query<RoomPayload>,
) -> Result<Json<GameState>, Error> {
    log::info!("received game_state request");
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::GameState {
            room: payload.room_id,
            responder,
        })
        .await?;
    receiver.await?.map(Json)
}

async fn hand_of_player(
    player: AuthenticatedPlayer,
    State(state): State<ServerState>,
) -> Result<Json<Vec<Card>>, Error> {
    log::info!("received hand request from player {}", player.player_id);
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::GetHand {
            player: player.player_id,
            room: player.room_id,
            responder,
        })
        .await?;
    receiver.await?.map(Json)
}

async fn last_move(
    State(state): State<ServerState>,
    Query(payload): Query<RoomPayload>,
) -> Result<Json<Action>, Error> {
    log::info!("received last move request");
    let (responder, receiver) = oneshot::channel();
    state
        .sender
        .send(RouterServerMessage::LastMove {
            room: payload.room_id,
            responder,
        })
        .await?;
    receiver.await?.map(Json)
}

#[derive(Debug, Serialize)]
struct JoinSuccess {
    token_type: String,
    token: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct RoomPayload {
    room_id: Uuid,
}

#[derive(Debug, Deserialize)]
struct NewRoomRequest {
    players: usize,
    decks: usize,
}

/// Represents a player that has been verified based on their PASETO token.
#[derive(Debug, Serialize)]
pub struct AuthenticatedPlayer {
    token: String,
    pub player_id: usize,
    pub room_id: Uuid,
}

impl ServerState {
    /// Verify that the `token` is a valid PASETO token signed by us and create
    /// an `AuthenticatedPlayer` based on it.
    fn verify(&self, token: &str) -> Result<AuthenticatedPlayer, Error> {
        let untrusted_token =
            pasetors::token::UntrustedToken::<pasetors::Public, V4>::try_from(token)
                .map_err(|_| Error::InvalidToken)?;
        let validation_rules = ClaimsValidationRules::new();
        let trusted_token = pasetors::public::verify(
            &self.key_pair.public,
            &untrusted_token,
            &validation_rules,
            None,
            None,
        )
        .map_err(|_| Error::InvalidToken)?;
        let player = AuthenticatedPlayer {
            token: token.to_owned(),
            player_id: trusted_token
                .payload_claims()
                .unwrap()
                .get_claim("sub")
                .unwrap()
                .as_str()
                .unwrap()
                .parse()
                .unwrap(),
            room_id: serde_json::from_value::<Uuid>(
                trusted_token
                    .payload_claims()
                    .unwrap()
                    .get_claim("room_id")
                    .unwrap()
                    .clone(),
            )
            .unwrap(),
        };
        Ok(player)
    }
}

#[async_trait]
impl FromRequestParts<ServerState> for AuthenticatedPlayer {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &ServerState,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| Error::InvalidToken)?;
        state.verify(token.token())
    }
}
