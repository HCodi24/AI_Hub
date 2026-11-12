//! MCP servers MutiS itself runs for in-app agents (API sources such as DeepSeek). The Codex and Claude CLIs keep
//! using their own MCP configuration; these entries are never written into their config files.
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// `[a-z0-9_-]{1,32}`: becomes part of the tool names the model sees.
    pub name: String,
    /// Executable (`codegraph`, a full path, ...). Started directly, never through a shell line.
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default = "enabled")]
    pub enabled: bool,
}

fn enabled() -> bool {
    true
}

pub const MAX_MCP_SERVERS: usize = 20;

/// Checks a whole list the user submitted.
pub fn validate_mcp_servers(servers: &[McpServerConfig]) -> Result<()> {
    ensure!(servers.len() <= MAX_MCP_SERVERS, "Tối đa {MAX_MCP_SERVERS} MCP server");
    let mut seen = HashSet::new();
    for server in servers {
        let valid_name = (1..=32).contains(&server.name.len())
            && server.name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
        if !valid_name {
            bail!("Tên MCP chỉ gồm a-z, 0-9, - và _ (tối đa 32 ký tự): {}", server.name);
        }
        ensure!(seen.insert(server.name.as_str()), "Tên MCP bị trùng: {}", server.name);
        ensure!(!server.command.trim().is_empty(), "MCP {} chưa có lệnh chạy", server.name);
        ensure!(server.args.len() <= 50 && server.env.len() <= 50, "MCP {} có quá nhiều tham số", server.name);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(name: &str, command: &str) -> McpServerConfig {
        McpServerConfig { name: name.into(), command: command.into(), enabled: true, ..Default::default() }
    }

    #[test]
    fn accepts_a_normal_list_and_rejects_bad_names_duplicates_and_empty_commands() {
        assert!(validate_mcp_servers(&[server("codegraph", "codegraph"), server("my_tool-2", "x")]).is_ok());
        assert!(validate_mcp_servers(&[server("Bad Name", "x")]).is_err());
        assert!(validate_mcp_servers(&[server("a", "x"), server("a", "y")]).is_err());
        assert!(validate_mcp_servers(&[server("a", "  ")]).is_err());
    }

    #[test]
    fn missing_enabled_defaults_to_on() {
        let parsed: McpServerConfig = serde_json::from_str(r#"{"name":"a","command":"b"}"#).unwrap();
        assert!(parsed.enabled);
    }
}
