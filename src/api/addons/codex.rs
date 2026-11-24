use super::{bad, cli::run_cli_full, executable, first_provider};
use crate::{api::ApiResult, app::AppState, config::ProviderConfig};
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::path::PathBuf;

async fn codex_provider(state: &AppState) -> ApiResult<ProviderConfig> {
    first_provider(state, "codex_cli")
        .await
        .ok_or_else(|| bad("MutiS chưa có nguồn Codex"))
}

/// The Codex CLI and AI Hub's own Codex home (never `~/.codex`).
pub(super) async fn codex_cli(state: &AppState) -> ApiResult<(PathBuf, PathBuf)> {
    let provider = codex_provider(state).await?;
    let codex = executable(state, "codex_cli", "codex").await?;
    let home = crate::providers::codex::environment::home(&state.config_path, &provider)
        .map_err(|error| bad(error.to_string()))?;
    Ok((codex, home))
}

/// `codex plugin list --json` -> its `installed` entries; `home` picks AI Hub's Codex, `None` the user's own.
/// Any failure (no Codex, no plugins) is an empty list.
pub(super) async fn codex_list(codex: &std::path::Path, home: Option<&std::path::Path>) -> Vec<Value> {
    let Ok(text) = run_cli_full(codex, &["plugin", "list", "--json"], false, home).await else {
        return Vec::new();
    };
    // Codex may print a warning line before the JSON.
    text.find('{')
        .and_then(|start| serde_json::from_str::<Value>(&text[start..]).ok())
        .and_then(|json| json["installed"].as_array().cloned())
        .unwrap_or_default()
}

/// Marketplace of OpenAI's account-wide plugins: every ChatGPT login gets them and they cannot be uninstalled.
const CODEX_DEFAULT_MARKETPLACE: &str = "openai-curated-remote";

/// True for a plugin Codex installs on its own for the signed-in account (an explicit flag when Codex reports one,
/// otherwise its marketplace).
pub(super) fn codex_default_plugin(plugin: &Value) -> bool {
    let flagged = ["installedByDefault", "installed_by_default", "isDefault", "default"]
        .iter()
        .any(|key| plugin[*key].as_bool() == Some(true));
    flagged
        || plugin["pluginId"]
            .as_str()
            .and_then(|id| id.rsplit_once('@'))
            .is_some_and(|(_, market)| market == CODEX_DEFAULT_MARKETPLACE || market.starts_with("openai-bundled") || market == "openai-primary-runtime")
}

/// Plugins in AI Hub's Codex, and the ones only the user's own Codex has (what "Thêm" can copy over).
pub(crate) async fn codex_plugins(State(state): State<AppState>) -> Json<Value> {
    let (mut installed, mut available) = (Vec::new(), Vec::new());
    if let Ok((codex, home)) = codex_cli(&state).await {
        installed = codex_list(&codex, Some(&home)).await;
        // `codex plugin list` can come back short on another machine; the cache folder is the ground truth.
        for cached in crate::providers::codex::environment::cached_plugins(&home) {
            if !installed.iter().any(|p| p["pluginId"] == cached["pluginId"]) { installed.push(cached); }
        }
        available = codex_list(&codex, None).await;
    }
    // Plugins the account installs by default cannot be removed (the server answers 403), so they are not listed.
    installed.retain(|plugin| !codex_default_plugin(plugin));
    available.retain(|plugin| !codex_default_plugin(plugin));
    let view = |plugins: &[Value]| -> Vec<Value> {
        plugins
            .iter()
            .filter_map(|plugin| {
                Some(json!({
                    "id": plugin["pluginId"].as_str()?,
                    "version": plugin["version"].as_str().unwrap_or_default(),
                    "enabled": plugin["enabled"].as_bool().unwrap_or(true),
                }))
            })
            .collect()
    };
    let have: Vec<&str> = installed
        .iter()
        .filter_map(|plugin| plugin["pluginId"].as_str())
        .collect();
    available.retain(|plugin| {
        plugin["pluginId"]
            .as_str()
            .is_some_and(|id| !have.contains(&id))
    });
    let mcp = match codex_cli(&state).await {
        Ok((_, home)) => crate::providers::codex::environment::mcp_servers(&home),
        Err(_) => Vec::new(),
    };
    Json(json!({ "plugins": view(&installed), "available": view(&available), "mcp": mcp }))
}
