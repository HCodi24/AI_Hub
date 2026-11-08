use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_folder")]
    pub default_folder: String,
}

fn default_folder() -> String {
    r"C:\TOOL".to_owned()
}

impl Config {
    pub fn seeded() -> Self {
        Self {
            providers: vec![
                ProviderConfig {
                    id: "deploy-deepseek".to_owned(),
                    name: "Deploy (DeepSeek)".to_owned(),
                    kind: "openai_compat".to_owned(),
                    base_url: "https://api.deploy.com/api/v1".to_owned(),
                    api_key: String::new(),
                    models: vec!["deepseek-ai/deepseek-v4.1-flash-1017".to_owned()],
                },
                ProviderConfig {
                    id: "openai".to_owned(),
                    name: "OpenAI".to_owned(),
                    kind: "openai_compat".to_owned(),
                    base_url: "https://api.openai.com/v1".to_owned(),
                    api_key: String::new(),
                    models: vec!["gpt-5.1".to_owned()],
                },
                ProviderConfig {
                    id: "anthropic".to_owned(),
                    name: "Anthropic".to_owned(),
                    kind: "anthropic".to_owned(),
                    base_url: "https://api.anthropic.com".to_owned(),
                    api_key: String::new(),
                    models: vec!["claude-opus-5-5".to_owned(), "claude-sonnet-5-5".to_owned()],
                },
                ProviderConfig {
                    id: "codex-cli".to_owned(),
                    name: "Codex CLI".to_owned(),
                    kind: "codex_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    models: vec!["gpt-6-luna".to_owned()],
                },
                ProviderConfig {
                    id: "claude-cli".to_owned(),
                    name: "Claude Code CLI".to_owned(),
                    kind: "claude_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    models: vec!["opus".to_owned(), "sonnet".to_owned()],
                },
            ],
            default_folder: default_folder(),
        }
    }
}

pub fn config_file_path() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("AI_HUB_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }
    let root =
        dirs::config_dir().context("Could not find the operating system config directory")?;
    Ok(root.join("ai-hub").join("config.json"))
}

pub fn load_or_seed(path: &Path) -> Result<Config> {
    if path.exists() {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Could not read config file at {}", path.display()))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Could not parse config file at {}", path.display()))
    } else {
        let config = Config::seeded();
        save(path, &config)?;
        Ok(config)
    }
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

pub fn masked_key(key: &str) -> String {
    if key.is_empty() {
        return String::new();
    }
    if key.chars().count() <= 4 {
        return "••••".to_owned();
    }
    let suffix: String = key
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    if key.starts_with("sk-") {
        format!("sk-…{suffix}")
    } else {
        format!("…{suffix}")
    }
}

pub fn is_masked_key(value: &str) -> bool {
    value.contains('…') || value.contains("••") || value.starts_with("****")
}
