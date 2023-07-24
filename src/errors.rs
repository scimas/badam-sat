use axum::{http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use serde_json::json;

#[derive(Debug, Serialize)]
pub struct InvalidToken;

impl IntoResponse for InvalidToken {
    fn into_response(self) -> axum::response::Response {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

#[derive(Debug, Serialize)]
pub struct JoinFail {
    error: String,
}

impl IntoResponse for JoinFail {
    fn into_response(self) -> axum::response::Response {
        (StatusCode::CONFLICT, Json(self)).into_response()
    }
}

impl JoinFail {
    pub fn new(error: String) -> Self {
        JoinFail { error }
    }
}

#[derive(Debug, Serialize)]
pub enum ClientError {
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
