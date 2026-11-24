use crate::features::providers::error::{ProviderError, ProviderErrorKind};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

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

impl From<ProviderError> for ApiError {
    fn from(error: ProviderError) -> Self {
        let status = match error.kind {
            ProviderErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ProviderErrorKind::NotFound => StatusCode::NOT_FOUND,
            ProviderErrorKind::BadGateway => StatusCode::BAD_GATEWAY,
            ProviderErrorKind::GatewayTimeout => StatusCode::GATEWAY_TIMEOUT,
            ProviderErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        Self::new(status, error.message)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;
