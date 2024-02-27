use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;
use tokio::sync::{mpsc, oneshot};

#[derive(Debug, Serialize, thiserror::Error)]
pub enum Error {
    #[error("attempted move is not valid")]
    InvalidMove,
    #[error("game is not ready to accept moves yet")]
    TooEarly,
    #[error("user not authorized")]
    InvalidToken,
    #[error("no such room exists")]
    InvalidRoomId,
    #[error("cannot join a full room")]
    RoomFull,
    #[error("no such player exists")]
    InvalidPlayerId,
    #[error("no space left in the server for another game")]
    ServerFull,
    #[error("no last move found")]
    NoMove,
    #[error("game server or room terminated unexpectedly")]
    UnexpectedTermination,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let response_code = match self {
            Error::InvalidMove => StatusCode::BAD_REQUEST,
            Error::TooEarly => StatusCode::BAD_REQUEST,
            Error::InvalidToken => StatusCode::UNAUTHORIZED,
            Error::InvalidRoomId => StatusCode::BAD_REQUEST,
            Error::RoomFull => StatusCode::BAD_REQUEST,
            Error::InvalidPlayerId => StatusCode::BAD_REQUEST,
            Error::ServerFull => StatusCode::CONFLICT,
            Error::NoMove => StatusCode::NOT_FOUND,
            Error::UnexpectedTermination => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (response_code, Json(json!({"error": self.to_string()}))).into_response()
    }
}

impl<T> From<mpsc::error::SendError<T>> for Error {
    fn from(_: mpsc::error::SendError<T>) -> Self {
        Error::UnexpectedTermination
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(_: oneshot::error::RecvError) -> Self {
        Error::UnexpectedTermination
    }
}
