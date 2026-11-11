use super::{migrate, Config};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn config_file_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AI_HUB_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }
    let root =
        dirs::config_dir().context("Could not find the operating system config directory")?;
    Ok(root.join("ai-hub").join("config.json"))
}

pub fn load_or_seed(path: &Path) -> Result<Config> {
    let mut config: Config = if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Could not read config file at {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Could not parse config file at {}", path.display()))?
    } else {
        Config::seeded()
    };
    migrate::merge_agent_providers(&mut config);
    migrate::migrate_mutis(&mut config);
    migrate::add_codex_cli_model_suggestions(&mut config);
    save(path, &config)?;
    Ok(config)
}

pub fn save(path: &Path, config: &Config) -> Result<()> {
    let parent = path
        .parent()
        .context("Config file has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Could not create config directory {}", parent.display()))?;
    let json = serde_json::to_string_pretty(config).context("Could not encode config")?;
    fs::write(path, json).with_context(|| format!("Could not save config to {}", path.display()))
}
