//! Which CLIs are installed and where to find them.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
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
    if !["codex", "claude"].contains(&name.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CLI kh\u{00f4}ng h\u{1ee3}p l\u{1ec7}",
        ));
    }
    Ok(Json(discovery::discover_candidates(&name).await))
}

pub(crate) async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
    let config = state.config.read().await;
    let codex_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
    let claude_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "claude_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
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

async fn cli_info(name: &str, explicit_path: String) -> CliInfo {
    let (installed, version, path) = discovery::probe_cli_info(name, &explicit_path).await;
    CliInfo {
        installed,
        version,
        path,
    }
}
