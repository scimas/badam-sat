use std::{
    collections::HashMap, fs::File, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration,
};

use axum::{
    async_trait,
    extract::{FromRequestParts, State},
    headers::{authorization::Bearer, Authorization},
    http::{request::Parts, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, RequestPartsExt, Router, TypedHeader,
};
use axum_server::tls_rustls::RustlsConfig;
use badam_sat::games::{BadamSat, PlayingArea, Transition};
use card_deck::standard_deck::Card;
use clap::Parser;
use pasetors::{
    claims::{Claims, ClaimsValidationRules},
    keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey},
    version4::V4,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use simple_logger::SimpleLogger;
use tokio::sync::{watch, RwLock};
use tower_http::services::ServeDir;

/// बदाम सात game server
#[derive(Debug, Parser)]
#[command(author = "scimas", version, about, long_about = None)]
struct Args {
    /// Number of players for the game
    #[arg(long, default_value_t = 4)]
    players: usize,

    /// Number of 52-card decks to play the game with
    #[arg(long, default_value_t = 1)]
    decks: usize,

    /// Path to the signing key for token generation
    ///
    /// This must be an ED25519 key.
    #[arg(long)]
    signing_key: String,

    /// Address for the server
    #[arg(long, default_value = "127.0.0.1:8080")]
    address: String,

    /// Use TLS
    #[arg(long)]
    secure: bool,

    /// Path to the directory containing the TLS key and certificate
    ///
    /// Required when using the `--secure` option
    #[arg(long)]
    tls_dir: Option<String>,
}

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .init()
        .unwrap();
    let args = Args::parse();

    let mut sign_key_file = File::open(&args.signing_key).unwrap();
    let paseto_key = read_key_pair(&mut sign_key_file).unwrap();

    let game = BadamSat::with_player_and_deck_capacity(args.players, args.decks);
    let (play_area_sender, _) = watch::channel(game.playing_area().clone());
    let (winner_sender, _) = watch::channel(json!({}));
    let server = Server {
        key_pair: paseto_key,
        players: HashMap::with_capacity(args.players),
        tokens: Vec::with_capacity(args.players),
        game,
        max_player_count: args.players,
        play_area_sender,
        winner_sender,
    };

    let serve_dir = ServeDir::new("dist");
    let router = Router::new()
        .route("/api/join", post(join))
        .route("/api/play", post(play))
        .route("/api/playing_area", get(playing_area))
        .route("/api/my_hand", get(hand_of_player))
        .route("/api/winner", get(winner))
        .fallback_service(serve_dir)
        .with_state(Arc::new(RwLock::new(server)));

    let address: SocketAddr = args.address.parse().unwrap();

    if args.secure {
        let tls_dir = args
            .tls_dir
            .expect("`--tls-dir` needs to be specified when using `--secure`");
        let tls_config = RustlsConfig::from_pem_file(
            PathBuf::from(&tls_dir).join("cert.pem"),
            PathBuf::from(&tls_dir).join("key.pem"),
        )
        .await
        .unwrap();
        axum_server::bind_rustls(address, tls_config)
            .serve(router.into_make_service())
            .await
            .unwrap();
    } else {
        axum::Server::bind(&address)
            .serve(router.into_make_service())
            .await
            .unwrap();
    };
}

fn read_key_pair<T: std::io::Read>(reader: &mut T) -> std::io::Result<AsymmetricKeyPair<V4>> {
    let mut key_data = String::new();
    reader.read_to_string(&mut key_data).unwrap();
    let key = ed25519_compact::KeyPair::from_pem(&key_data).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::Other,
            "could not deserialize key from key data",
        )
    })?;
    let sk = AsymmetricSecretKey::<V4>::from(key.sk.as_ref()).expect("could not create secret key");
    let pk = AsymmetricPublicKey::<V4>::from(key.pk.as_ref()).expect("could not create public key");
    let paseto_key = AsymmetricKeyPair {
        secret: sk,
        public: pk,
    };
    Ok(paseto_key)
}

struct Server {
    key_pair: AsymmetricKeyPair<V4>,
    // mapping from PASETO token to player index
    players: HashMap<String, usize>,
    // PASETO tokens for player indices
    tokens: Vec<String>,
    game: BadamSat,
    max_player_count: usize,
    play_area_sender: watch::Sender<PlayingArea>,
    winner_sender: watch::Sender<serde_json::Value>,
}

impl Server {
    fn verify(&self, token: &str) -> Result<usize, InvalidToken> {
        let untrusted_token =
            pasetors::token::UntrustedToken::<pasetors::Public, V4>::try_from(token)
                .map_err(|_| InvalidToken)?;
        let validation_rules = ClaimsValidationRules::new();
        let trusted_token = pasetors::public::verify(
            &self.key_pair.public,
            &untrusted_token,
            &validation_rules,
            None,
            None,
        )
        .map_err(|_| InvalidToken)?;
        Ok(trusted_token
            .payload_claims()
            .unwrap()
            .get_claim("sub")
            .unwrap()
            .as_str()
            .unwrap()
            .parse()
            .unwrap())
    }

    fn join(&mut self) -> Result<String, JoinFail> {
        if self.is_full() {
            return Err(JoinFail {
                error: "server is full".into(),
            });
        }
        let mut claim = Claims::new().unwrap();
        claim.subject(&self.players.len().to_string()).unwrap();
        let token = pasetors::public::sign(&self.key_pair.secret, &claim, None, None).unwrap();
        let current_player = self.players.len();
        self.players.insert(token.clone(), current_player);
        self.tokens.push(token.clone());
        if self.players.len() == self.max_player_count {
            self.game.update(Transition::DealCards).unwrap();
        }
        Ok(token)
    }

    fn is_full(&self) -> bool {
        self.max_player_count == self.players.len()
    }

    fn play(&mut self, action: Action, player: usize) -> Result<(), ClientError> {
        if !self.is_full() {
            return Err(ClientError::TooEarly);
        }
        let transition = match action {
            Action::Play(card) => Transition::Play { player, card },
            Action::Pass => Transition::Pass { player },
        };
        match self.game.update(transition) {
            Ok(_) => {
                self.play_area_sender
                    .send_replace(self.playing_area().clone());
                if let Some(id) = self.game.winner() {
                    self.winner_sender.send_replace(json!({ "id": id }));
                }
                Ok(())
            }
            Err(_) => Err(ClientError::InvalidMove),
        }
    }

    fn playing_area(&self) -> &PlayingArea {
        self.game.playing_area()
    }

    fn hand_of_player(&self, player: usize) -> Vec<Card> {
        self.game.hand_of_player(player).to_vec()
    }
}

async fn join(State(server): State<Arc<RwLock<Server>>>) -> Result<Json<JoinSuccess>, JoinFail> {
    log::info!("received join request");
    server.write().await.join().map(|token| {
        Json(JoinSuccess {
            token_type: "Bearer".into(),
            token,
        })
    })
}

async fn play(
    player_id: PlayerId,
    State(server): State<Arc<RwLock<Server>>>,
    Json(action): Json<Action>,
) -> Result<StatusCode, ClientError> {
    log::info!("received play request from player {}", player_id.id);
    server
        .write()
        .await
        .play(action, player_id.id)
        .map(|_| StatusCode::OK)
}

async fn playing_area(State(server): State<Arc<RwLock<Server>>>) -> Json<PlayingArea> {
    log::info!("received playing_area request");
    let mut receiver = server.read().await.play_area_sender.subscribe();
    let play_area = {
        tokio::select! {
            _ = receiver.changed() => (),
            _ = tokio::time::sleep(Duration::from_secs(10)) => ()
        };
        receiver.borrow().clone()
    };
    Json(play_area)
}

async fn hand_of_player(
    player_id: PlayerId,
    State(server): State<Arc<RwLock<Server>>>,
) -> Json<Vec<Card>> {
    log::info!("received hand request from player {}", player_id.id);
    Json(server.read().await.hand_of_player(player_id.id))
}

async fn winner(State(server): State<Arc<RwLock<Server>>>) -> Json<serde_json::Value> {
    log::info!("received winner request");
    let mut receiver = server.read().await.winner_sender.subscribe();
    let play_area = {
        receiver.changed().await.unwrap();
        receiver.borrow().clone()
    };
    Json(play_area)
}

#[derive(Debug, Serialize)]
struct PlayerId {
    token: String,
    id: usize,
}

#[derive(Debug, Deserialize)]
enum Action {
    Play(Card),
    Pass,
}

#[derive(Debug, Serialize)]
enum ClientError {
    InvalidMove,
    TooEarly,
}

impl IntoResponse for ClientError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ClientError::InvalidMove => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "attempted move is not valid"})),
            )
                .into_response(),
            ClientError::TooEarly => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "game is not ready to accept moves yet"})),
            )
                .into_response(),
        }
    }
}

#[async_trait]
impl FromRequestParts<Arc<RwLock<Server>>> for PlayerId {
    type Rejection = InvalidToken;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<RwLock<Server>>,
    ) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(token)) = parts
            .extract::<TypedHeader<Authorization<Bearer>>>()
            .await
            .map_err(|_| InvalidToken)?;
        state
            .read()
            .await
            .verify(token.token())
            .map(|player| PlayerId {
                id: player,
                token: token.token().to_owned(),
            })
    }
}

#[derive(Debug, Serialize)]
struct JoinSuccess {
    token_type: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct JoinFail {
    error: String,
}

impl IntoResponse for JoinFail {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::CONFLICT, Json(self)).into_response()
    }
}

#[derive(Debug, Serialize)]
struct InvalidToken;

impl IntoResponse for InvalidToken {
    fn into_response(self) -> axum::response::Response {
        StatusCode::UNAUTHORIZED.into_response()
    }
}
