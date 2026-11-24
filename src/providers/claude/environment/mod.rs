mod auth;
mod launch;

pub use auth::{import_auth_file, import_cli_auth};
pub use launch::{enabled_hub_mcp, launch_args};

use crate::config::{ClaudeConfig, ProviderConfig};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

pub fn home(config: &Path, provider: &ProviderConfig) -> Result<PathBuf> {
    if !crate::providers::cli_support::valid_provider_id(&provider.id) {
        bail!("Mã nguồn Claude không hợp lệ");
    }
    let data = crate::core::paths::data_dir(config)?;
    let dir = data.join("claude").join(&provider.id);
    std::fs::create_dir_all(&dir)?;
    // Migrate only the original default source, once. Never restore credentials after logout.
    let marker = dir.join(".mutis-migrated");
    if !marker.exists() {
        if provider.id == "claude-cli" && !dir.join(".credentials.json").exists() {
            let old = data.join("claude-config/.credentials.json");
            if old.is_file() {
                import_auth_file(&old, &dir)?;
            }
        }
        std::fs::write(marker, b"1")?;
    }
    Ok(dir)
}

pub fn settings() -> Result<ClaudeConfig> {
    let config: crate::config::Config =
        serde_json::from_slice(&std::fs::read(crate::config::config_file_path()?)?)?;
    Ok(config.claude)
}

pub fn session_prefix(settings: &ClaudeConfig, provider: &ProviderConfig, folder: &str) -> String {
    let text = format!(
        "{}:{}:{}:{}:{}:{}",
        settings.revision,
        settings.project_skills,
        settings.project_instructions,
        provider.id,
        provider.executable,
        folder
    );
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let hash: String = digest.as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("mutis-claude-{hash}:")
}

pub fn apply(command: &mut tokio::process::Command, home: &Path) {
    crate::providers::cli_support::clean_claude_environment(command);
    command.env("CLAUDE_CONFIG_DIR", home);
}

#[cfg(test)]
mod tests;
