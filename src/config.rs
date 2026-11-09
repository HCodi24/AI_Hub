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
    pub executable: String,
    #[serde(default)]
    pub models: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_folder")]
    pub default_folder: String,
    #[serde(default)]
    pub planner_provider_id: String,
    #[serde(default)]
    pub planner_model: String,
    #[serde(default)]
    pub orchestration: OrchestrationConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrchestrationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub retry_api: bool,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_base_ms")]
    pub retry_base_ms: u64,
    #[serde(default)]
    pub codex_watchdog: bool,
    #[serde(default = "default_watchdog_minutes")]
    pub watchdog_minutes: u32,
    #[serde(default)]
    pub codex_auto_resume: bool,
    #[serde(default = "default_resume_max")]
    pub resume_max: u32,
    #[serde(default)]
    pub deepseek_agent: bool,
    #[serde(default = "default_max_file_lines")]
    pub deepseek_max_file_lines: u32,
    #[serde(default)]
    pub verify_gate: bool,
    #[serde(default)]
    pub verify_command: String,
    #[serde(default)]
    pub auto_review: bool,
    #[serde(default)]
    pub reviewer_provider_id: String,
    #[serde(default)]
    pub reviewer_model: String,
    #[serde(default)]
    pub auto_fix: bool,
    #[serde(default = "default_fix_rounds")]
    pub fix_rounds_max: u32,
    #[serde(default = "default_event_log")]
    pub event_log: bool,
    #[serde(default = "default_routing")]
    pub routing: Vec<RoutingRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub kind: String,
    pub assignee_kind: String,
    #[serde(default)]
    pub note: String,
}

impl Default for OrchestrationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            retry_api: false,
            retry_attempts: 3,
            retry_base_ms: 2000,
            codex_watchdog: false,
            watchdog_minutes: 5,
            codex_auto_resume: false,
            resume_max: 2,
            deepseek_agent: false,
            deepseek_max_file_lines: 400,
            verify_gate: false,
            verify_command: String::new(),
            auto_review: false,
            reviewer_provider_id: String::new(),
            reviewer_model: String::new(),
            auto_fix: false,
            fix_rounds_max: 2,
            event_log: true,
            routing: default_routing(),
        }
    }
}
impl OrchestrationConfig {
    pub fn effective(&self, flag: bool) -> bool {
        self.enabled && flag
    }
}
fn default_retry_attempts() -> u32 {
    3
}
fn default_retry_base_ms() -> u64 {
    2000
}
fn default_watchdog_minutes() -> u32 {
    5
}
fn default_resume_max() -> u32 {
    2
}
fn default_max_file_lines() -> u32 {
    400
}
fn default_fix_rounds() -> u32 {
    2
}
fn default_event_log() -> bool {
    true
}
fn default_routing() -> Vec<RoutingRule> {
    [
        ("small_edit", "deepseek_agent"),
        ("docs", "deepseek_agent"),
        ("code", "codex_cli"),
        ("test", "codex_cli"),
        ("plan", "claude_cli"),
        ("review", "claude_cli"),
    ]
    .into_iter()
    .map(|(kind, assignee_kind)| RoutingRule {
        kind: kind.into(),
        assignee_kind: assignee_kind.into(),
        note: String::new(),
    })
    .collect()
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
                    executable: String::new(),
                    models: vec!["deepseek-ai/deepseek-v4.1-flash-1017".to_owned()],
                },
                ProviderConfig {
                    id: "openai".to_owned(),
                    name: "OpenAI".to_owned(),
                    kind: "openai_compat".to_owned(),
                    base_url: "https://api.openai.com/v1".to_owned(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["gpt-5.1".to_owned()],
                },
                ProviderConfig {
                    id: "anthropic".to_owned(),
                    name: "Anthropic".to_owned(),
                    kind: "anthropic".to_owned(),
                    base_url: "https://api.anthropic.com".to_owned(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["claude-opus-5-5".to_owned(), "claude-sonnet-5-5".to_owned()],
                },
                ProviderConfig {
                    id: "codex-cli".to_owned(),
                    name: "Codex CLI".to_owned(),
                    kind: "codex_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["gpt-6-luna".to_owned()],
                },
                ProviderConfig {
                    id: "claude-cli".to_owned(),
                    name: "Claude Code CLI".to_owned(),
                    kind: "claude_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["opus".to_owned(), "sonnet".to_owned()],
                },
            ],
            default_folder: default_folder(),
            planner_provider_id: String::new(),
            planner_model: String::new(),
            orchestration: OrchestrationConfig::default(),
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
