use anyhow::{Context, Result};
use serde_json::json;
use std::path::Path;

/// `[mcp_servers.NAME]` table header -> NAME (plain or quoted).
fn mcp_header(line: &str) -> Option<&str> {
    let name = line.trim().strip_prefix("[mcp_servers.")?.strip_suffix(']')?;
    let name = name.trim_matches('"');
    (!name.contains('.') && !name.is_empty()).then_some(name)
}

/// MCP servers in the Codex home's `config.toml` as `{name, summary, enabled}` (Codex itself loads them; `enabled = false` turns one off).
pub fn mcp_servers(home: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let (mut out, mut current) = (Vec::<serde_json::Value>::new(), None::<usize>);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            current = mcp_header(line).map(|name| {
                out.push(json!({"name": name, "summary": "", "enabled": true}));
                out.len() - 1
            });
        } else if let Some(i) = current {
            if let Some(value) = line
                .strip_prefix("command")
                .and_then(|v| v.trim_start().strip_prefix('='))
            {
                out[i]["summary"] = json!(value.trim().trim_matches('"'));
            }
            if line.replace(' ', "") == "enabled=false" {
                out[i]["enabled"] = json!(false);
            }
        }
    }
    out
}

/// Writes `enabled = true|false` into one `[mcp_servers.NAME]` table of the Codex home's `config.toml`.
pub fn set_mcp_enabled(home: &Path, name: &str, on: bool) -> Result<()> {
    let path = home.join("config.toml");
    let text = std::fs::read_to_string(&path).context("Chưa có config.toml của Codex")?;
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let start = lines
        .iter()
        .position(|l| mcp_header(l) == Some(name))
        .with_context(|| format!("Codex chưa có MCP «{name}»"))?;
    let end = lines[start + 1..]
        .iter()
        .position(|l| l.trim().starts_with('['))
        .map_or(lines.len(), |i| start + 1 + i);
    let line = format!("enabled = {on}");
    match (start + 1..end).find(|&i| lines[i].replace(' ', "").starts_with("enabled=")) {
        Some(i) => lines[i] = line,
        None => lines.insert(start + 1, line),
    }
    std::fs::write(&path, lines.join("\n") + "\n")?;
    Ok(())
}
