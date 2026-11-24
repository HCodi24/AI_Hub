use super::{Home, McpServer, Plugin};
use super::super::paths::read_json;
use serde_json::Value;
use std::path::Path;

/// A Claude Code config: `dir` is the config folder, `state` the `.claude.json` that holds its user MCP servers.
pub(in crate::features::addons) fn claude_home(dir: &Path, state: &Path) -> Home {
    Home {
        path: dir.to_string_lossy().into_owned(),
        plugins: read_plugins(dir),
        skills: read_skills(dir),
        mcp: read_mcp(state),
    }
}

fn read_plugins(dir: &Path) -> Vec<Plugin> {
    let installed = read_json(&dir.join("plugins/installed_plugins.json"));
    let marketplaces = read_json(&dir.join("plugins/known_marketplaces.json"));
    let settings = read_json(&dir.join("settings.json"));
    let mut plugins = Vec::new();
    if let Some(map) = installed["plugins"].as_object() {
        for (id, entries) in map {
            let version = entries[0]["version"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            let marketplace = id
                .rsplit_once('@')
                .map(|(_, name)| name)
                .unwrap_or_default();
            plugins.push(Plugin {
                id: id.clone(),
                version,
                enabled: settings["enabledPlugins"][id].as_bool().unwrap_or(true),
                marketplace_source: marketplace_source(&marketplaces[marketplace]),
            });
        }
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    plugins
}

fn read_skills(dir: &Path) -> Vec<String> {
    let mut skills = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join("skills")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            // `synced` is managed by the Claude account itself, not something to copy around.
            if name != "synced" && entry.path().join("SKILL.md").is_file() {
                skills.push(name);
            }
        }
    }
    skills.sort();
    skills
}

fn read_mcp(state: &Path) -> Vec<McpServer> {
    let mut mcp = Vec::new();
    if let Some(map) = read_json(state)["mcpServers"].as_object() {
        for (name, server) in map {
            mcp.push(McpServer {
                name: name.clone(),
                summary: mcp_summary(server),
                enabled: false,
            });
        }
    }
    mcp.sort_by(|a, b| a.name.cmp(&b.name));
    mcp
}

fn marketplace_source(entry: &Value) -> String {
    let source = &entry["source"];
    ["repo", "url", "path"]
        .iter()
        .find_map(|key| source[*key].as_str())
        .unwrap_or_default()
        .to_owned()
}

fn mcp_summary(server: &Value) -> String {
    if let Some(url) = server["url"].as_str() {
        return url.to_owned();
    }
    let mut parts = vec![server["command"].as_str().unwrap_or_default().to_owned()];
    if let Some(args) = server["args"].as_array() {
        parts.extend(
            args.iter()
                .filter_map(|arg| arg.as_str().map(str::to_owned)),
        );
    }
    parts.join(" ").trim().to_owned()
}
