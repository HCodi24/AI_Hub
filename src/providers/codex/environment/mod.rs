use crate::config::{CodexConfig, ProviderConfig};
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};

mod auth;
mod launch;
mod mcp;
mod plugins;
mod project;

pub use auth::import_cli_auth;
pub use launch::{has_plugins, launch_args};
pub use mcp::{mcp_servers, set_mcp_enabled};
pub use plugins::{cached_plugins, plugin_index};
pub use project::project_context;

#[cfg(test)]
mod tests;

pub fn home(config: &Path, provider: &ProviderConfig) -> Result<PathBuf> {
    if !crate::providers::cli_support::valid_provider_id(&provider.id) {
        bail!("Mã nguồn Codex không hợp lệ");
    }
    let codex_root = crate::core::paths::data_dir(config)?.join("codex");
    let path = codex_root.join(&provider.id);
    std::fs::create_dir_all(&path)?;
    crate::features::providers::sandbox::codex::share_sandbox_state(&codex_root, &path);
    Ok(path)
}

pub fn settings() -> Result<CodexConfig> {
    let path = crate::config::config_file_path()?;
    let config: crate::config::Config = serde_json::from_slice(&std::fs::read(path)?)?;
    Ok(config.codex)
}

pub fn session_prefix(settings: &CodexConfig, provider: &ProviderConfig, folder: &str) -> String {
    let text = format!(
        "{}:{}:{}:{}:{}:{}",
        settings.revision,
        settings.project_skills,
        settings.project_instructions,
        provider.id,
        provider.executable,
        folder
    );
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let key = hash.as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("mutis-codex-{key}:")
}

pub fn project_root(folder: &Path) -> PathBuf {
    folder
        .ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(folder)
        .to_path_buf()
}
