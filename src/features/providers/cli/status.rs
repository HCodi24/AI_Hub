//! Which CLIs are installed and where to find them.
use super::discovery;
use crate::{
    app::AppState,
    config::Config,
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
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

pub(crate) async fn discover_cli(name: &str) -> ProviderResult<Vec<discovery::CliCandidate>> {
    if !["codex", "claude", "agy"].contains(&name) {
        return Err(ProviderError::new(
            ProviderErrorKind::BadRequest,
            "CLI không hợp lệ",
        ));
    }
    Ok(discovery::discover_candidates(name).await)
}

pub(crate) async fn cli_status(state: &AppState) -> CliStatus {
    let config = state.config.read().await;
    let codex_executable = executable_of(&config, "codex_cli");
    let claude_executable = executable_of(&config, "claude_cli");
    drop(config);
    let (codex_cli, claude_cli) = tokio::join!(
        cli_info("codex", codex_executable),
        cli_info("claude", claude_executable)
    );
    CliStatus {
        codex_cli,
        claude_cli,
    }
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
