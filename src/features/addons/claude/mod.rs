//! Two homes exist on a machine:
//! - `personal`: the user's own Claude Code (`~/.claude` + `~/.claude.json`);
//! - `hub`: AI Hub's Claude Code (`<data>/claude-config`), a separate login, so it starts with no add-ons.
//!
//! Plugin and MCP changes go through the CLIs. Skill folders are copied directly.

use super::{
    paths::read_json,
    skills::{builtin_skill_names, shared_skill_names},
};
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

mod home;
mod skills;

pub(super) use home::claude_home;
pub use skills::{copy_skill, remove_skill};

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
    /// Whether chats load it; filled in by the API from the app config, never read from the file.
    pub enabled: bool,
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
    pub builtin_skills: Vec<String>,
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
        builtin_skills: builtin_skill_names(),
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
