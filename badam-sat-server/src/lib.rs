use std::{path::Path, sync::Arc, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use badam_sat::games::PlayingArea;
use card_deck::standard_deck::Card;
use errors::Error;
use pasetors::{keys::AsymmetricKeyPair, version4::V4};
use rooms::{Action, Winner};
use serde::{Deserialize, Serialize};
use server::{AuthenticatedPlayer, Server};
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::errors::ServerError;

mod errors;
mod rooms;
mod server;

/// Create a router for बदाम सात.
pub fn badam_sat_router<P: AsRef<Path>>(
    key_pair: AsymmetricKeyPair<V4>,
    max_rooms: usize,
    frontend_path: P,
) -> (Router, Arc<RwLock<Server>>) {
    let server = Arc::new(RwLock::new(Server::new(key_pair, max_rooms)));

    let serve_dir = ServeDir::new(frontend_path);
    let router = Router::new()
        .route("/api/create_room", post(create_room))
        .route("/api/join", post(join))
        .route("/api/play", post(play))
        .route("/api/playing_area", get(playing_area))
        .route("/api/my_hand", get(hand_of_player))
        .route("/api/winner", get(winner))
        .route("/api/last_move", get(last_move))
        .fallback_service(serve_dir)
        .with_state(server.clone());

    (router, server)
}

async fn create_room(
    State(server): State<Arc<RwLock<Server>>>,
    Json(room_request): Json<NewRoomRequest>,
) -> Result<Json<RoomPayload>, Error> {
    log::info!("received create room request");
    server
        .write()
        .await
        .create_room(room_request.players, room_request.decks)
        .map(|room_id| Json(RoomPayload { room_id }))
}

async fn join(
    State(server): State<Arc<RwLock<Server>>>,
    Json(payload): Json<RoomPayload>,
) -> Result<Json<JoinSuccess>, Error> {
    log::info!("received join request");
    server.write().await.join(&payload.room_id).map(|token| {
        Json(JoinSuccess {
            token_type: "Bearer".into(),
            token,
        })
    })
}

async fn play(
    player: AuthenticatedPlayer,
    State(server): State<Arc<RwLock<Server>>>,
    Json(action): Json<Action>,
) -> Result<StatusCode, Error> {
    log::info!("received play request from player {}", player.player_id);
    server
        .write()
        .await
        .play(action, player.player_id, &player.room_id)
        .map(|_| StatusCode::OK)
}

async fn playing_area(
    State(server): State<Arc<RwLock<Server>>>,
    Query(payload): Query<RoomPayload>,
) -> Result<Json<PlayingArea>, Error> {
    log::info!("received playing_area request");
    let mut receiver = server
        .read()
        .await
        .room(&payload.room_id)?
        .play_area_sender()
        .subscribe();
    let play_area = {
        tokio::select! {
            _ = receiver.changed() => (),
            _ = tokio::time::sleep(Duration::from_secs(10)) => ()
        };
        receiver.borrow().clone()
    };
    Ok(Json(play_area))
}

async fn hand_of_player(
    player: AuthenticatedPlayer,
    State(server): State<Arc<RwLock<Server>>>,
) -> Result<Json<Vec<Card>>, Error> {
    log::info!("received hand request from player {}", player.player_id);
    server
        .read()
        .await
        .room(&player.room_id)?
        .hand_of_player(player.player_id)
        .map(|cards| Json(cards.to_vec()))
}

async fn winner(
    State(server): State<Arc<RwLock<Server>>>,
    Query(payload): Query<RoomPayload>,
) -> Result<Json<Winner>, Error> {
    log::info!("received winner request");
    let mut receiver = server
        .read()
        .await
        .room(&payload.room_id)?
        .winner_sender()
        .subscribe();
    let play_area = {
        receiver.changed().await.unwrap();
        *receiver.borrow()
    };
    Ok(Json(play_area))
}

async fn last_move(
    State(server): State<Arc<RwLock<Server>>>,
    Query(payload): Query<RoomPayload>,
) -> Result<Json<Action>, Error> {
    log::info!("received last move request");
    if let Some(action) = server.read().await.room(&payload.room_id)?.last_move() {
        Ok(Json(*action))
    } else {
        Err(Error::ServerError(ServerError::NoMove))
    }
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
