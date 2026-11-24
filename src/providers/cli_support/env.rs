use super::auth::auth_origin;
use anyhow::Result;
use std::{env, path::Path};
use tokio::process::Command;

pub fn environment_names_to_remove(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            let upper = name.to_ascii_uppercase();
            upper.starts_with("CLAUDE") || upper.starts_with("ANTHROPIC_")
        })
        .cloned()
        .collect()
}

/// agy's own home for one source. A source signed in through the web also gets `SSH_CONNECTION`, which makes agy keep its login in this
/// home (`.gemini/antigravity-cli/antigravity-oauth-token`) instead of the OS keyring every agy on the machine shares.
pub fn agy_environment(command: &mut Command, home: &Path) {
    command.env("USERPROFILE", home).env("HOME", home);
    if auth_origin(home).as_deref() == Some("web") {
        command.env("SSH_CONNECTION", "127.0.0.1 1 127.0.0.1 22");
    }
}

pub fn clean_claude_environment(command: &mut Command) {
    let names = env::vars().map(|(name, _)| name).collect::<Vec<_>>();
    for name in super::environment_names_to_remove(&names) {
        command.env_remove(name);
    }
    // AI Hub's Claude Code CLI gets its own config dir (login, settings, history), separate from the user's
    // `~/.claude` used by the Claude desktop app, so the two can be signed in to different accounts.
    if let Some(dir) = ai_hub_claude_config_dir() {
        command.env("CLAUDE_CONFIG_DIR", dir);
    }
}

/// `<AI Hub data dir>\claude-config` (created on demand), e.g. `%APPDATA%\ai-hub\claude-config`.
pub fn ai_hub_claude_config_dir() -> Option<std::path::PathBuf> {
    let config_path = crate::core::config::config_file_path().ok()?;
    let dir = claude_config_dir(&config_path).ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn claude_config_dir(config_path: &Path) -> Result<std::path::PathBuf> {
    Ok(crate::core::paths::data_dir(config_path)?.join("claude-config"))
}
