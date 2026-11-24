//! Web sign-in for Claude Code CLI and Antigravity accounts.

mod code;
mod parse;
mod start;
#[cfg(test)]
mod tests;

pub(crate) use code::login_provider_code;
pub(crate) use start::login_provider_start;

use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct LoginStartResponse {
    pub(crate) session_id: String,
    url: String,
}

pub(crate) struct LoginCodeOutcome {
    pub(crate) exited: bool,
    pub(crate) message: String,
    pub(crate) email: Option<String>,
    pub(crate) output_tail: String,
}

fn cli_label(kind: &str) -> &'static str {
    if kind == "antigravity_cli" {
        "Antigravity CLI"
    } else {
        "Claude CLI"
    }
}

pub(crate) async fn login_provider_config(state: &AppState, id: &str) -> ProviderResult<ProviderConfig> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ProviderError::new(ProviderErrorKind::NotFound, "Không tìm thấy nguồn AI"))?;
    if provider.kind != "claude_cli" && !(provider.kind == "antigravity_cli" && provider.web_login) {
        return Err(ProviderError::new(
            ProviderErrorKind::BadRequest,
            "Chỉ Claude Code CLI và tài khoản Web của Antigravity mới đăng nhập được",
        ));
    }
    Ok(provider)
}

pub(crate) async fn login_provider_cancel(state: &AppState, id: &str) -> ProviderResult<()> {
    login_provider_config(state, id).await?;
    let mut active = state.claude_login.lock().await;
    if active
        .as_ref()
        .is_some_and(|session| session.provider_id == id)
    {
        if let Some(mut session) = active.take() {
            let _ = session.child.kill().await;
        }
    }
    Ok(())
}
