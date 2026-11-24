use crate::providers::cli_support;
use anyhow::Result;
use serde_json::{json, Value};

pub(crate) const SIGNED_OUT_FILE: &str = ".mutis-signed-out";

/// Session ids saved by another source or folder must never be handed to `--conversation`.
pub(crate) fn session_prefix(provider: &crate::config::ProviderConfig, folder: &str, settings: &crate::config::AntigravityConfig) -> String {
    let mut text = format!("agy:{}:{}:{}", provider.id, provider.executable, folder);
    if settings.project_skills || settings.project_instructions {
        text.push_str(&format!(":project-context:{}:{}", settings.project_skills, settings.project_instructions));
    }
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let key = hash.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("mutis-agy-{key}:")
}

/// Clean per-source home (`<data>/antigravity/<id>`): agy keeps its chats, plugins, settings and MCP list under `~/.gemini`,
/// so pointing USERPROFILE/HOME here keeps them apart from the user's own agy. The login lives in the OS keyring and still works.
/// Keeps the home's login mode in step with the source: a web source marks it "web" (see `cli_support::agy_environment`).
pub fn sync_login_mode(provider: &crate::config::ProviderConfig) {
    if provider.kind == "antigravity_cli" {
        if let Ok(home) = clean_home(&provider.id) {
            cli_support::set_auth_origin(&home, provider.web_login.then_some("web"));
        }
    }
}

pub fn clean_home(provider_id: &str) -> Result<std::path::PathBuf> {
    let id: String = provider_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let dir = crate::core::paths::data_dir(&crate::core::config::config_file_path()?)?.join("antigravity").join(id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// AI Hub's MCP list (and CodeGraph) becomes the clean home's `mcp_config.json`, rewritten every turn.
pub(super) fn write_mcp_config(home: &std::path::Path, servers: &[crate::config::McpServerConfig]) -> Result<()> {
    let list: serde_json::Map<String, Value> = servers
        .iter()
        .filter(|s| s.enabled)
        .map(|s| (s.name.clone(), json!({"command": s.command, "args": s.args, "env": s.env})))
        .collect();
    let dir = home.join(".gemini").join("config");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("mcp_config.json"), serde_json::to_vec_pretty(&json!({"mcpServers": list}))?)?;
    Ok(())
}
