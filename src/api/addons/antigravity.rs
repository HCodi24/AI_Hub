use super::{bad, cli::run_cli_full, executable, first_provider};
use crate::{api::ApiResult, app::AppState, features::addons};
use axum::{extract::State, Json};
use serde_json::{json, Value};
use std::path::PathBuf;

/// The Antigravity CLI and the clean home chats run with.
pub(super) async fn agy_cli(state: &AppState) -> ApiResult<(PathBuf, PathBuf)> {
    let provider = first_provider(state, "antigravity_cli")
        .await
        .ok_or_else(|| bad("MutiS chưa có nguồn Antigravity"))?;
    let agy = executable(state, "antigravity_cli", "agy").await?;
    let home = crate::providers::antigravity_cli::clean_home(&provider.id).map_err(|error| bad(error.to_string()))?;
    Ok((agy, home))
}

/// `(name, install folder, version)` of the plugins installed in the user's own Claude Code.
pub(super) fn claude_plugin_paths() -> Vec<(String, String, String)> {
    let Some(file) = addons::personal_claude_dir().map(|dir| dir.join("plugins").join("installed_plugins.json")) else { return Vec::new() };
    let Some(json) = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) else { return Vec::new() };
    json["plugins"].as_object().into_iter().flatten().filter_map(|(id, entries)| {
        let entry = entries.as_array()?.first()?;
        Some((id.split('@').next()?.to_owned(), entry["installPath"].as_str()?.to_owned(), entry["version"].as_str().unwrap_or_default().to_owned()))
    }).collect()
}

/// Plugins in AI Hub's Antigravity, and the Claude Code plugins that "Thêm" can import into it.
pub(crate) async fn antigravity_plugins(State(state): State<AppState>) -> Json<Value> {
    let Ok((agy, home)) = agy_cli(&state).await else { return Json(json!({"plugins": [], "available": []})) };
    let installed: Vec<Value> = match run_cli_full(&agy, &["plugin", "list", "--json"], false, Some(&home)).await {
        Ok(text) => text.find('[').and_then(|start| serde_json::from_str(&text[start..]).ok()).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let names: Vec<&str> = installed.iter().filter_map(|plugin| plugin["name"].as_str()).collect();
    let available: Vec<Value> = claude_plugin_paths().into_iter()
        .filter(|(name, ..)| !names.contains(&name.as_str()))
        .map(|(name, _, version)| json!({"id": name, "version": version, "enabled": true}))
        .collect();
    let plugins: Vec<Value> = installed.iter().filter_map(|plugin| Some(json!({
        "id": plugin["name"].as_str()?, "version": plugin["version"].as_str().unwrap_or_default(),
        "enabled": plugin["enabled"].as_bool().unwrap_or(true),
    }))).collect();
    Json(json!({"plugins": plugins, "available": available}))
}
