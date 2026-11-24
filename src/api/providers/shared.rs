use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
};
use axum::http::StatusCode;
use std::sync::atomic::Ordering::SeqCst;

pub(super) async fn find_provider(
    state: &AppState,
    id: &str,
    kind: Option<&str>,
    not_found: &'static str,
) -> ApiResult<ProviderConfig> {
    let config = state.config.read().await;
    config
        .providers
        .iter()
        .find(|provider| provider.id == id && kind.is_none_or(|kind| provider.kind == kind))
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, not_found))
}

pub(crate) use crate::features::providers::login_mode::ensure_login_mode;

pub(super) fn ensure_idle(state: &AppState, message: &'static str) -> ApiResult<()> {
    if state.active_jobs.load(SeqCst) > 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, message));
    }
    Ok(())
}
