//! "Test connection" for every source kind, with a 60 second cache.
mod cache;

pub(crate) use self::cache::{cached_provider_test, test_provider};

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::providers::login_key::api_check,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
pub(crate) use crate::features::providers::connection::{connection, ConnectionTest};

pub(crate) async fn test_provider_uncached(
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

#[cfg(test)]
mod tests;
