//! "Test connection" for every source kind, with a 60 second cache.
mod api_check;
mod cache;

pub(crate) use self::cache::{cached_provider_test, test_provider};

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

#[derive(Clone, Serialize)]
pub(crate) struct ConnectionTest {
    ok: bool,
    pub(crate) status: &'static str,
    message: String,
    detail: String,
    /// The account's plan, for sources that report one (Antigravity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
}

pub(super) async fn test_provider_uncached(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    let result = match provider.kind.as_str() {
        "openai_compat" => api_check::openai(&provider).await,
        "anthropic" => api_check::anthropic(&provider).await,
        "codex_cli" => super::codex::test_connection(&state, &id).await,
        "claude_cli" => super::claude::test_connection(&state, &id).await,
        "antigravity_cli" => super::antigravity::test_connection(&provider).await,
        _ => api_check::connection_error("Loại nhà cung cấp không được hỗ trợ"),
    };
    Ok(Json(result))
}

pub(super) fn connection(ok: bool, status: &'static str, message: &str, detail: &str) -> ConnectionTest {
    ConnectionTest {
        ok,
        status,
        message: message.into(),
        detail: detail.into(),
        plan: None,
        email: None,
    }
}

#[cfg(test)]
mod tests;
