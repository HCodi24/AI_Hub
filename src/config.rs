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
    #[serde(alias = "orchestration")]
    pub mutis: MutisConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutisConfig {
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
    #[serde(default = "default_deepseek_max_steps")]
    pub deepseek_max_steps: u32,
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
    #[serde(default)]
    pub roles: MutisRoles,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRoles {
    pub lead: MutisRole,
    pub big: MutisRole,
    pub small: MutisRole,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRole {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub kind: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub assignee_kind: String,
    #[serde(default)]
    pub note: String,
}

impl Default for MutisConfig {
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
            deepseek_max_steps: 15,
            verify_gate: false,
            verify_command: String::new(),
            auto_review: false,
            reviewer_provider_id: String::new(),
            reviewer_model: String::new(),
            auto_fix: false,
            fix_rounds_max: 2,
            event_log: true,
            routing: default_routing(),
            roles: MutisRoles::default(),
        }
    }
}
impl MutisConfig {
    pub fn effective(&self, flag: bool) -> bool {
        self.enabled && flag
    }
}
fn default_deepseek_max_steps() -> u32 {
    15
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
        ("small_edit", "small"),
        ("docs", "small"),
        ("code", "big"),
        ("test", "big"),
        ("plan", "lead"),
        ("review", "lead"),
    ]
    .into_iter()
    .map(|(kind, role)| RoutingRule {
        kind: kind.into(),
        role: role.into(),
        assignee_kind: String::new(),
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
                    id: "deepseek-agent-deploy".to_owned(),
                    name: "DeepSeek làm việc (Deploy)".to_owned(),
                    kind: "deepseek_agent".to_owned(),
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
            mutis: MutisConfig::default(),
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
        let mut config: Config = serde_json::from_str(&content)
            .with_context(|| format!("Could not parse config file at {}", path.display()))?;
        if !config
            .providers
            .iter()
            .any(|provider| provider.id == "deepseek-agent-deploy")
        {
            let seeded = Config::seeded();
            if let Some(provider) = seeded
                .providers
                .into_iter()
                .find(|provider| provider.id == "deepseek-agent-deploy")
            {
                config.providers.push(provider);
            }
        }
        migrate_mutis(&mut config);
        save(path, &config)?;
        Ok(config)
    } else {
        let mut config = Config::seeded();
        migrate_mutis(&mut config);
        save(path, &config)?;
        Ok(config)
    }
}

fn default_effort() -> String {
    "default".into()
}
fn migrate_mutis(config: &mut Config) {
    let mut roles = config.mutis.roles.clone();
    let find = |kinds: &[&str]| {
        config
            .providers
            .iter()
            .find(|p| kinds.contains(&p.kind.as_str()))
            .map(|p| p.id.clone())
            .unwrap_or_default()
    };
    if roles.lead.provider_id.is_empty() {
        roles.lead.provider_id = if !config.mutis.reviewer_provider_id.is_empty() {
            config.mutis.reviewer_provider_id.clone()
        } else if !config.planner_provider_id.is_empty() {
            config.planner_provider_id.clone()
        } else {
            find(&["claude_cli", "anthropic", "codex_cli"])
        };
    }
    if roles.lead.effort.is_empty() {
        roles.lead.effort = "default".into();
    }
    if roles.lead.model.is_empty() {
        roles.lead.model = if !config.mutis.reviewer_model.is_empty() {
            config.mutis.reviewer_model.clone()
        } else if !config.planner_model.is_empty() {
            config.planner_model.clone()
        } else {
            config
                .providers
                .iter()
                .find(|p| p.id == roles.lead.provider_id)
                .and_then(|p| p.models.first())
                .cloned()
                .unwrap_or_default()
        };
    }
    if roles.big.provider_id.is_empty() {
        roles.big.provider_id = find(&["codex_cli"]);
    }
    if roles.big.model.is_empty() {
        roles.big.model = config
            .providers
            .iter()
            .find(|p| p.id == roles.big.provider_id)
            .and_then(|p| p.models.first())
            .cloned()
            .unwrap_or_default();
    }
    if roles.big.effort.is_empty() {
        roles.big.effort = "high".into();
    }
    if roles.small.provider_id.is_empty() {
        roles.small.provider_id = config
            .providers
            .iter()
            .find(|p| p.id == "deepseek-agent-deploy")
            .map(|p| p.id.clone())
            .unwrap_or_else(|| find(&["deepseek_agent"]));
    }
    if roles.small.model.is_empty() {
        roles.small.model = config
            .providers
            .iter()
            .find(|p| p.id == roles.small.provider_id)
            .and_then(|p| p.models.first())
            .cloned()
            .unwrap_or_default();
    }
    if roles.small.effort.is_empty() {
        roles.small.effort = "default".into();
    }
    let fallback_provider = config.providers.first();
    for role in [&mut roles.lead, &mut roles.big, &mut roles.small] {
        if role.provider_id.is_empty() {
            if let Some(provider) = fallback_provider {
                role.provider_id = provider.id.clone();
            }
        }
        if role.model.is_empty() {
            role.model = config
                .providers
                .iter()
                .find(|provider| provider.id == role.provider_id)
                .and_then(|provider| provider.models.first())
                .cloned()
                .unwrap_or_default();
        }
    }
    for rule in &mut config.mutis.routing {
        if rule.role.is_empty() {
            rule.role = match rule.assignee_kind.as_str() {
                "deepseek_agent" => "small",
                "codex_cli" => "big",
                "claude_cli" => "lead",
                _ => "lead",
            }
            .into();
        }
    }
    config.mutis.roles = roles;
    config.planner_provider_id = config.mutis.roles.lead.provider_id.clone();
    config.planner_model = config.mutis.roles.lead.model.clone();
    config.mutis.reviewer_provider_id = config.mutis.roles.lead.provider_id.clone();
    config.mutis.reviewer_model = config.mutis.roles.lead.model.clone();
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

#[cfg(test)]
mod role_tests {
    use super::*;
    #[test]
    fn legacy_planner_reviewer_and_routing_migrate_to_roles() {
        let mut config = Config::seeded();
        config.planner_provider_id = "codex-cli".into();
        config.planner_model = "gpt-6-luna".into();
        config.mutis.reviewer_provider_id = "claude-cli".into();
        config.mutis.reviewer_model = "opus".into();
        config.mutis.routing[0].role.clear();
        config.mutis.routing[0].assignee_kind = "deepseek_agent".into();
        config.mutis.routing[2].role.clear();
        config.mutis.routing[2].assignee_kind = "codex_cli".into();
        config.mutis.routing[4].role.clear();
        config.mutis.routing[4].assignee_kind = "claude_cli".into();
        migrate_mutis(&mut config);
        assert_eq!(config.mutis.roles.lead.provider_id, "claude-cli");
        assert_eq!(config.mutis.roles.lead.model, "opus");
        assert_eq!(config.mutis.routing[0].role, "small");
        assert_eq!(config.mutis.routing[2].role, "big");
        assert_eq!(config.mutis.routing[4].role, "lead");
    }

    #[test]
    fn migration_fills_small_with_deepseek_agent_and_first_model() {
        let mut config = Config::seeded();
        config.mutis.roles.small.provider_id.clear();
        config.mutis.roles.small.model.clear();
        migrate_mutis(&mut config);
        assert_eq!(
            config.mutis.roles.small.provider_id,
            "deepseek-agent-deploy"
        );
        assert_eq!(
            config.mutis.roles.small.model,
            "deepseek-ai/deepseek-v4.1-flash-1017"
        );
    }

    #[test]
    fn migration_never_leaves_roles_without_provider_when_any_provider_exists() {
        let mut config = Config::seeded();
        config.providers.retain(|provider| provider.id == "openai");
        config.mutis.roles = MutisRoles::default();
        migrate_mutis(&mut config);
        assert_eq!(config.mutis.roles.lead.provider_id, "openai");
        assert_eq!(config.mutis.roles.big.provider_id, "openai");
        assert_eq!(config.mutis.roles.small.provider_id, "openai");
    }

    #[test]
    fn fallback_uses_only_confirmed_connected_providers_in_role_order() {
        let config = Config::seeded();
        let connected = ["codex-cli".to_owned()].into_iter().collect();
        assert_eq!(
            first_connected_fallback(&config.providers, "lead", &connected).as_deref(),
            Some("codex-cli")
        );
        assert_eq!(
            first_connected_fallback(&config.providers, "big", &connected).as_deref(),
            Some("codex-cli")
        );
        assert_eq!(
            first_connected_fallback(&config.providers, "small", &connected).as_deref(),
            Some("codex-cli")
        );
        let unknown = std::collections::HashSet::new();
        assert_eq!(
            first_connected_fallback(&config.providers, "lead", &unknown),
            None
        );
    }
}

pub fn first_connected_fallback(
    providers: &[ProviderConfig],
    role: &str,
    connected: &std::collections::HashSet<String>,
) -> Option<String> {
    let kinds: &[&str] = match role {
        "lead" => &["claude_cli", "anthropic", "codex_cli"],
        "big" => &["codex_cli", "claude_cli"],
        "small" => &["deepseek_agent", "codex_cli"],
        _ => return None,
    };
    kinds.iter().find_map(|kind| {
        providers
            .iter()
            .find(|provider| provider.kind == *kind && connected.contains(&provider.id))
            .map(|provider| provider.id.clone())
    })
}

#[allow(clippy::items_after_test_module)]
pub fn is_masked_key(value: &str) -> bool {
    value.contains('…') || value.contains("••") || value.starts_with("****")
}
