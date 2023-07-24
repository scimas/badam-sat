use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
pub enum Error {
    ClientError(ClientError),
    // ServerError(ServerError),
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        match self {
            Error::ClientError(client_error) => client_error.into_response(),
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ClientError {
    InvalidMove,
    TooEarly,
    InvalidToken,
    InvalidRoomId,
    RoomFull,
    InvalidPlayerId,
    ServerFull,
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
            ClientError::InvalidToken => StatusCode::UNAUTHORIZED.into_response(),
            ClientError::InvalidRoomId => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no such room exists"})),
            )
                .into_response(),
            ClientError::RoomFull => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "cannot join a full room"})),
            )
                .into_response(),
            ClientError::InvalidPlayerId => (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "no such player exists"})),
            )
                .into_response(),
            ClientError::ServerFull => (
                StatusCode::CONFLICT,
                Json(json!({"error": "no space left in server for another game"})),
            )
                .into_response(),
        }
    }
}
