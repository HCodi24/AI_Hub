//! Which CLIs are installed and where to find them.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::Config,
    providers::discovery,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct CliStatus {
    codex_cli: CliInfo,
    claude_cli: CliInfo,
}

#[derive(Serialize)]
struct CliInfo {
    installed: bool,
    version: Option<String>,
    path: Option<String>,
}

pub(crate) async fn discover_cli(
    Path(name): Path<String>,
) -> ApiResult<Json<Vec<discovery::CliCandidate>>> {
    if !["codex", "claude", "agy"].contains(&name.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CLI không hợp lệ",
        ));
    }
    Ok(Json(discovery::discover_candidates(&name).await))
}

pub(crate) async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
    let config = state.config.read().await;
    let codex_executable = executable_of(&config, "codex_cli");
    let claude_executable = executable_of(&config, "claude_cli");
    drop(config);
    let (codex_cli, claude_cli) = tokio::join!(
        cli_info("codex", codex_executable),
        cli_info("claude", claude_executable)
    );
    Json(CliStatus {
        codex_cli,
        claude_cli,
    })
}

fn executable_of(config: &Config, kind: &str) -> String {
    config
        .providers
        .iter()
        .find(|provider| provider.kind == kind)
        .map(|provider| provider.executable.clone())
        .unwrap_or_default()
}

async fn cli_info(name: &str, explicit_path: String) -> CliInfo {
    let (installed, version, path) = discovery::probe_cli_info(name, &explicit_path).await;
    CliInfo {
        installed,
        version,
        path,
    }
}
