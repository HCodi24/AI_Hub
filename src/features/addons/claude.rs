//! Two homes exist on a machine:
//! - `personal`: the user's own Claude Code (`~/.claude` + `~/.claude.json`);
//! - `hub`: AI Hub's Claude Code (`<data>/claude-config`), a separate login, so it starts with no add-ons.
//!
//! Plugin and MCP changes go through the CLIs. Skill folders are copied directly.

use super::{
    paths::{read_json, valid_name},
    skills::{shared_skill_names, skill_install_lock},
};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct Plugin {
    pub id: String,
    pub version: String,
    pub enabled: bool,
    /// GitHub `owner/repo` (or URL/path) of the marketplace the plugin came from, when known.
    pub marketplace_source: String,
}

#[derive(Serialize, Default, Clone, Debug, PartialEq)]
pub struct McpServer {
    pub name: String,
    /// `command args…` or the URL — never env values or headers.
    pub summary: String,
}

#[derive(Serialize, Default, Clone, Debug)]
pub struct Home {
    pub path: String,
    pub plugins: Vec<Plugin>,
    pub skills: Vec<String>,
    pub mcp: Vec<McpServer>,
}

#[derive(Serialize, Default, Clone, Debug)]
pub struct Overview {
    pub personal: Home,
    pub hub: Home,
    pub shared_skills: Vec<String>,
}

fn marketplace_source(entry: &Value) -> String {
    let source = &entry["source"];
    ["repo", "url", "path"]
        .iter()
        .find_map(|key| source[*key].as_str())
        .unwrap_or_default()
        .to_owned()
}

/// A Claude Code config: `dir` is the config folder, `state` the `.claude.json` that holds its user MCP servers.
pub(super) fn claude_home(dir: &Path, state: &Path) -> Home {
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
    let mut mcp = Vec::new();
    if let Some(map) = read_json(state)["mcpServers"].as_object() {
        for (name, server) in map {
            mcp.push(McpServer {
                name: name.clone(),
                summary: mcp_summary(server),
            });
        }
    }
    mcp.sort_by(|a, b| a.name.cmp(&b.name));
    Home {
        path: dir.to_string_lossy().into_owned(),
        plugins,
        skills,
        mcp,
    }
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

pub fn personal_claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude"))
}

pub(crate) fn personal_claude_home() -> Home {
    let home = dirs::home_dir().unwrap_or_default();
    claude_home(&home.join(".claude"), &home.join(".claude.json"))
}

pub fn overview(hub_dir: &Path, skills_dir: &Path) -> Overview {
    Overview {
        personal: personal_claude_home(),
        hub: claude_home(hub_dir, &hub_dir.join(".claude.json")),
        shared_skills: shared_skill_names(skills_dir),
    }
}

/// The JSON definition of one of the user's own MCP servers (with its env), for `claude mcp add-json`.
pub fn personal_mcp_definition(name: &str) -> Result<Value> {
    let home = dirs::home_dir().context("Không tìm được thư mục người dùng")?;
    let server = read_json(&home.join(".claude.json"))["mcpServers"][name].clone();
    if !server.is_object() {
        bail!("Không tìm thấy MCP server «{name}» trong Claude Code của bạn");
    }
    Ok(server)
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// Copy one skill folder from the user's Claude Code into AI Hub's Claude config (replacing an older copy).
pub fn copy_skill(personal_dir: &Path, hub_dir: &Path, name: &str) -> Result<()> {
    if !valid_name(name) || name == "synced" {
        bail!("Tên skill không hợp lệ");
    }
    let from = personal_dir.join("skills").join(name);
    if !from.join("SKILL.md").is_file() {
        bail!("Không tìm thấy skill «{name}» trong Claude Code của bạn");
    }
    let _guard = skill_install_lock()?;
    let to = hub_dir.join("skills").join(name);
    if to.exists() {
        std::fs::remove_dir_all(&to)?;
    }
    copy_dir(&from, &to).with_context(|| format!("Không chép được skill «{name}»"))
}

pub fn remove_skill(hub_dir: &Path, name: &str) -> Result<()> {
    if !valid_name(name) || name == "synced" {
        bail!("Tên skill không hợp lệ");
    }
    let _guard = skill_install_lock()?;
    let dir = hub_dir.join("skills").join(name);
    if !dir.join("SKILL.md").is_file() {
        bail!("MutiS không có skill «{name}»");
    }
    std::fs::remove_dir_all(dir).with_context(|| format!("Không xoá được skill «{name}»"))
}
