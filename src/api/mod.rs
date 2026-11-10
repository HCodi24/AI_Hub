use crate::app::AppState;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json, Router,
};
use serde_json::json;

pub mod chat;
pub mod config;
pub mod dev;
pub mod extensions;
pub mod mutis;
pub mod providers;
pub mod static_ui;
pub mod verify;

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;

pub fn router(state: AppState, port: u16) -> Router {
    crate::server::router(state, port)
}
