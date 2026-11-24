//! HTTP API for "Plugin & Skill" (see features/addons.rs).

mod antigravity;
mod change;
mod cli;
mod codex;
mod link;
#[cfg(test)]
mod tests;

pub(crate) use self::{antigravity::antigravity_plugins, codex::codex_plugins};
pub(crate) use change::change;
pub(crate) use link::{inspect_link, install_link, install_link_skill};

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
    features::addons,
    providers::cli_support,
};
use axum::{extract::State, http::StatusCode, Json};
use std::path::PathBuf;

pub(super) fn bad(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, message)
}

async fn first_provider(state: &AppState, kind: &str) -> Option<ProviderConfig> {
    state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.kind == kind)
        .cloned()
}

/// The Claude config folder chats run with (`<data>/claude/<source id>`): plugins, MCP and skills must live there to be seen.
pub(super) async fn claude_dir(state: &AppState) -> ApiResult<PathBuf> {
    match first_provider(state, "claude_cli").await {
        Some(provider) => crate::providers::claude::environment::home(&state.config_path, &provider)
            .map_err(|error| bad(error.to_string())),
        None => cli_support::ai_hub_claude_config_dir()
            .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không tạo được thư mục cấu hình Claude của MutiS")),
    }
}

pub(crate) async fn overview(State(state): State<AppState>) -> ApiResult<Json<addons::Overview>> {
    let error = || {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được danh sách plugin và skill",
        )
    };
    let dir = claude_dir(&state).await.map_err(|_| error())?;
    let skills_dir = addons::shared_skills_dir(&state.config_path).map_err(|_| error())?;
    let mut overview = tokio::task::spawn_blocking(move || addons::overview(&dir, &skills_dir))
        .await
        .map_err(|_| error())?;
    let enabled = state.config.read().await.claude.enabled_mcp.clone();
    for server in &mut overview.hub.mcp { server.enabled = enabled.contains(&server.name); }
    Ok(Json(overview))
}

/// The CLI configured for the first source of `kind`, or one found on the machine.
pub(super) async fn executable(state: &AppState, kind: &str, name: &str) -> ApiResult<PathBuf> {
    let explicit = first_provider(state, kind)
        .await
        .map(|provider| provider.executable);
    cli_support::resolve_executable(name, explicit.as_deref()).ok_or_else(|| {
        bad(format!(
            "Không tìm thấy {name} CLI trên máy (xem Cài đặt › Nguồn AI)"
        ))
    })
}
