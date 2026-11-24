//! Web sign-in for Claude Code CLI and Antigravity accounts.

mod code;
mod parse;
mod start;
#[cfg(test)]
mod tests;

pub(crate) use code::login_provider_code;
pub(crate) use start::login_provider_start;

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct LoginStartResponse {
    session_id: String,
    url: String,
}
#[derive(Deserialize)]
pub(crate) struct LoginCodeInput {
    session_id: String,
    code: String,
}
#[derive(Serialize)]
pub(crate) struct LoginCodeResponse {
    ok: bool,
    logged_in: bool,
    message: String,
    output_tail: String,
}

fn cli_label(kind: &str) -> &'static str {
    if kind == "antigravity_cli" {
        "Antigravity CLI"
    } else {
        "Claude CLI"
    }
}

async fn login_provider_config(state: &AppState, id: &str) -> ApiResult<ProviderConfig> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn AI"))?;
    if provider.kind != "claude_cli" && !(provider.kind == "antigravity_cli" && provider.web_login) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ Claude Code CLI và tài khoản Web của Antigravity mới đăng nhập được",
        ));
    }
    Ok(provider)
}

pub(crate) async fn login_provider_cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    login_provider_config(&state, &id).await?;
    let mut active = state.claude_login.lock().await;
    if active
        .as_ref()
        .is_some_and(|session| session.provider_id == id)
    {
        if let Some(mut session) = active.take() {
            let _ = session.child.kill().await;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}
