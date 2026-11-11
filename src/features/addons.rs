//! "Plugin & Skill": what Claude Code and Codex load when AI Hub runs them, and copying the user's own Claude Code
//! add-ons into AI Hub's separate Claude config.
//!
//! Three homes exist on a machine:
//! - `personal`: the user's own Claude Code (`~/.claude` + `~/.claude.json`);
//! - `hub`: AI Hub's Claude Code (`<data>/claude-config`), a separate login, so it starts with no add-ons;
//! - Codex (`~/.codex/config.toml`), which AI Hub runs as-is, so everything there is already loaded.
//!
//! Listing only reads files. Changes go through the CLIs themselves (`claude plugin|mcp`, `codex mcp`) so their config
//! formats stay theirs; only skill folders are copied directly. Secrets (env values, headers) are never returned.

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
    pub codex: Home,
}

/// Plugin ids look like `name@marketplace`; skill and MCP names are plain. Anything else is refused (they reach a
/// command line and a file path).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
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
pub fn claude_home(dir: &Path, state: &Path) -> Home {
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

/// Codex keeps plugins and MCP servers as `[plugins."id"]` / `[mcp_servers.name]` tables in config.toml. Only the
/// table names and a few plain keys are needed, so a small line reader is enough (no TOML dependency).
pub fn codex_home(dir: &Path) -> Home {
    let text = std::fs::read_to_string(dir.join("config.toml")).unwrap_or_default();
    let mut plugins: Vec<Plugin> = Vec::new();
    let mut mcp: Vec<McpServer> = Vec::new();
    #[derive(PartialEq)]
    enum Section {
        Plugin,
        Mcp,
        Other,
    }
    let mut section = Section::Other;
    for line in text.lines().map(str::trim) {
        if let Some(header) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            let unquote = |name: &str| {
                name.trim()
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_owned()
            };
            section = Section::Other;
            if let Some(name) = header.strip_prefix("plugins.") {
                plugins.push(Plugin {
                    id: unquote(name),
                    enabled: true,
                    ..Default::default()
                });
                section = Section::Plugin;
            } else if let Some(name) = header.strip_prefix("mcp_servers.") {
                // `[mcp_servers.x.env]` belongs to server x; it holds secrets and is skipped.
                if !name.contains('.') {
                    mcp.push(McpServer {
                        name: unquote(name),
                        summary: String::new(),
                    });
                    section = Section::Mcp;
                }
            }
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim());
        match section {
            Section::Plugin if key == "enabled" => {
                if let Some(plugin) = plugins.last_mut() {
                    plugin.enabled = value == "true";
                }
            }
            Section::Mcp if matches!(key, "command" | "url") => {
                if let Some(server) = mcp.last_mut() {
                    server.summary = value.trim_matches(|c| c == '"' || c == '\'').to_owned();
                }
            }
            _ => {}
        }
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    mcp.sort_by(|a, b| a.name.cmp(&b.name));
    Home {
        path: dir.to_string_lossy().into_owned(),
        plugins,
        skills: Vec::new(),
        mcp,
    }
}

pub fn personal_claude_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".claude"))
}

pub fn overview(hub_dir: &Path) -> Overview {
    let home = dirs::home_dir().unwrap_or_default();
    Overview {
        personal: claude_home(&home.join(".claude"), &home.join(".claude.json")),
        hub: claude_home(hub_dir, &hub_dir.join(".claude.json")),
        codex: codex_home(&home.join(".codex")),
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
    let dir = hub_dir.join("skills").join(name);
    if !dir.join("SKILL.md").is_file() {
        bail!("AI Hub không có skill «{name}»");
    }
    std::fs::remove_dir_all(dir).with_context(|| format!("Không xoá được skill «{name}»"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("ai-hub-addons-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn names_are_restricted_to_safe_characters() {
        for ok in [
            "ponytail@ponytail",
            "ui-ux-pro-max@ui-ux-pro-max-skill",
            "code_graph",
            "a.b",
        ] {
            assert!(valid_name(ok), "{ok}");
        }
        for bad in [
            "", "../x", "a b", "-flag", ".hidden", "a/b", "a\\b", "x;y", "a\"b",
        ] {
            assert!(!valid_name(bad), "{bad}");
        }
    }

    #[test]
    fn claude_home_lists_plugins_skills_and_mcp_without_secrets() {
        let dir = temp("claude");
        std::fs::create_dir_all(dir.join("plugins")).unwrap();
        std::fs::create_dir_all(dir.join("skills/graphify")).unwrap();
        std::fs::create_dir_all(dir.join("skills/synced/x")).unwrap();
        std::fs::create_dir_all(dir.join("skills/not-a-skill")).unwrap();
        std::fs::write(dir.join("skills/graphify/SKILL.md"), "# Tóm tắt đồ thị").unwrap();
        std::fs::write(
            dir.join("plugins/installed_plugins.json"),
            r#"{"plugins":{"ponytail@ponytail":[{"version":"5.3.3"}],"old@market":[{"version":"1"}]}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("plugins/known_marketplaces.json"),
            r#"{"ponytail":{"source":{"source":"github","repo":"DietrichGebert/ponytail"}}}"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("settings.json"),
            r#"{"enabledPlugins":{"old@market":false}}"#,
        )
        .unwrap();
        let state = dir.join(".claude.json");
        std::fs::write(
            &state,
            r#"{"mcpServers":{"codegraph":{"command":"codegraph","args":["serve","--mcp"],"env":{"KEY":"secret"}},
               "web":{"type":"http","url":"https://example.test/mcp","headers":{"Authorization":"Bearer secret"}}}}"#,
        )
        .unwrap();
        let home = claude_home(&dir, &state);
        assert_eq!(home.skills, vec!["graphify"]);
        assert_eq!(home.plugins.len(), 2);
        assert!(!home.plugins[0].enabled && home.plugins[0].id == "old@market");
        assert_eq!(
            home.plugins[1].marketplace_source,
            "DietrichGebert/ponytail"
        );
        assert_eq!(home.plugins[1].version, "5.3.3");
        assert_eq!(home.mcp[0].summary, "codegraph serve --mcp");
        assert_eq!(home.mcp[1].summary, "https://example.test/mcp");
        assert!(!serde_json::to_string(&home).unwrap().contains("secret"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn codex_home_reads_plugin_and_mcp_tables() {
        let dir = temp("codex");
        std::fs::write(
            dir.join("config.toml"),
            "model = \"gpt\"\n[plugins.\"pdf@openai-primary-runtime\"]\nenabled = true\n\
             [plugins.\"off@personal\"]\nenabled = false\n[mcp_servers.codegraph]\ncommand = \"codegraph\"\n\
             args = [\"serve\"]\n[mcp_servers.codegraph.env]\nTOKEN = \"secret\"\n[mcp_servers.web]\nurl = 'https://x.test'\n",
        )
        .unwrap();
        let home = codex_home(&dir);
        assert_eq!(home.plugins.len(), 2);
        assert!(!home.plugins[0].enabled && home.plugins[0].id == "off@personal");
        assert_eq!(
            home.mcp,
            vec![
                McpServer {
                    name: "codegraph".into(),
                    summary: "codegraph".into()
                },
                McpServer {
                    name: "web".into(),
                    summary: "https://x.test".into()
                }
            ]
        );
        assert!(!serde_json::to_string(&home).unwrap().contains("secret"));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn skills_are_copied_replaced_and_removed() {
        let personal = temp("personal");
        let hub = temp("hub");
        std::fs::create_dir_all(personal.join("skills/graphify/scripts")).unwrap();
        std::fs::write(personal.join("skills/graphify/SKILL.md"), "bản một").unwrap();
        std::fs::write(personal.join("skills/graphify/scripts/run.py"), "print(1)").unwrap();
        copy_skill(&personal, &hub, "graphify").unwrap();
        assert!(hub.join("skills/graphify/scripts/run.py").is_file());
        std::fs::write(personal.join("skills/graphify/SKILL.md"), "bản hai").unwrap();
        std::fs::remove_file(personal.join("skills/graphify/scripts/run.py")).unwrap();
        copy_skill(&personal, &hub, "graphify").unwrap();
        assert_eq!(
            std::fs::read_to_string(hub.join("skills/graphify/SKILL.md")).unwrap(),
            "bản hai"
        );
        assert!(!hub.join("skills/graphify/scripts/run.py").exists());
        assert!(copy_skill(&personal, &hub, "missing").is_err());
        assert!(copy_skill(&personal, &hub, "../x").is_err());
        remove_skill(&hub, "graphify").unwrap();
        assert!(!hub.join("skills/graphify").exists());
        assert!(remove_skill(&hub, "graphify").is_err());
        for dir in [personal, hub] {
            let _ = std::fs::remove_dir_all(dir);
        }
    }
}
