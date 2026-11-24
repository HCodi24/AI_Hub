use crate::config::McpServerConfig;
use serde_json::{json, Value};
use std::{ffi::OsString, path::Path};

/// Every plugin in the hub's `plugins/installed_plugins.json`, switched on: installing is the only switch.
pub(super) fn enabled_plugins(home: &Path) -> serde_json::Map<String, Value> {
    let installed = std::fs::read_to_string(home.join("plugins/installed_plugins.json")).ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok());
    installed.as_ref().and_then(|json| json["plugins"].as_object()).into_iter().flatten()
        .map(|(id, _)| (id.clone(), Value::Bool(true))).collect()
}

/// The servers of AI Hub's Claude `.claude.json` that the user switched on (full definition, so env and url stay intact).
pub fn enabled_hub_mcp(home: &Path, enabled: &[String]) -> serde_json::Map<String, Value> {
    let state = std::fs::read_to_string(home.join(".claude.json")).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
    state.as_ref().and_then(|json| json["mcpServers"].as_object()).into_iter().flatten()
        .filter(|(name, _)| enabled.contains(name)).map(|(name, server)| (name.clone(), server.clone())).collect()
}

pub fn launch_args(mcp: &[McpServerConfig], home: &Path, hub_mcp: serde_json::Map<String, Value>) -> Vec<OsString> {
    let mut servers = mcp
        .iter()
        .filter(|s| s.enabled)
        .map(|s| {
            (
                s.name.clone(),
                json!({"command":s.command,"args":s.args,"env":s.env}),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
    // The app's own servers win a name clash.
    for (name, server) in hub_mcp { servers.entry(name).or_insert(server); }
    vec!["--setting-sources".into(), "user".into(),
        "--strict-mcp-config".into(), "--mcp-config".into(), json!({"mcpServers":servers}).to_string().into(),
        "--settings".into(), json!({"disableAllHooks":true,"autoMemoryEnabled":false,"enabledPlugins":enabled_plugins(home),"claudeMdExcludes":["**"],"disableClaudeAiConnectors":true}).to_string().into()]
}
