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

/// CLI sources sign in one way only: machine sources reuse the installed CLI's account, web sources use the web.
pub(super) fn ensure_login_mode(provider: &ProviderConfig, web: bool) -> ApiResult<()> {
    if provider.web_login == web {
        return Ok(());
    }
    Err(ApiError::new(
        StatusCode::BAD_REQUEST,
        if provider.web_login {
            "Nguồn này đăng nhập qua web, không dùng tài khoản CLI trên máy"
        } else {
            "Nguồn này dùng tài khoản CLI trên máy; muốn đăng nhập web hãy thêm ở tab Tài khoản web"
        },
    ))
}

pub(super) fn ensure_idle(state: &AppState, message: &'static str) -> ApiResult<()> {
    if state.active_jobs.load(SeqCst) > 0 {
        return Err(ApiError::new(StatusCode::CONFLICT, message));
    }
    Ok(())
}
