use std::{fs::File, net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use axum_server::tls_rustls::RustlsConfig;
use badam_sat::games::PlayingArea;
use card_deck::standard_deck::Card;
use clap::Parser;
use errors::Error;
use pasetors::{
    keys::{AsymmetricKeyPair, AsymmetricPublicKey, AsymmetricSecretKey},
    version4::V4,
};
use rooms::{Action, Winner};
use serde::{Deserialize, Serialize};
use server::{AuthenticatedPlayer, Server};
use simple_logger::SimpleLogger;
use tokio::sync::RwLock;
use tower_http::services::ServeDir;
use uuid::Uuid;

use crate::errors::ServerError;

mod errors;
mod rooms;
mod server;

/// बदाम सात game server
#[derive(Debug, Parser)]
#[command(author = "scimas", version, about, long_about = None)]
struct Args {
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

    /// Maximum simultaneous game rooms the server is allowed to host
    #[arg(long, default_value_t = 1<<6)]
    max_rooms: usize,
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

    let server = Arc::new(RwLock::new(Server::new(paseto_key, args.max_rooms)));
    {
        let server = server.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(120)).await;
                server.write().await.remove_finished_rooms();
            }
        });
    }

    let serve_dir = ServeDir::new("dist");
    let badam_sat_router = Router::new()
        .route("/api/create_room", post(create_room))
        .route("/api/join", post(join))
        .route("/api/play", post(play))
        .route("/api/playing_area", get(playing_area))
        .route("/api/my_hand", get(hand_of_player))
        .route("/api/winner", get(winner))
        .route("/api/last_move", get(last_move))
        .fallback_service(serve_dir)
        .with_state(server.clone());

    let app_router = Router::new().nest("/badam_sat", badam_sat_router);

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
            .serve(app_router.into_make_service())
            .await
            .unwrap();
    } else {
        axum::Server::bind(&address)
            .serve(app_router.into_make_service())
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
