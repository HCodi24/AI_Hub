mod migrate;
mod mutis;
mod provider;
mod store;

pub use mutis::*;
pub use provider::*;
pub use store::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub memory: crate::features::memory::MemoryConfig,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_folder")]
    pub default_folder: String,
    #[serde(default = "default_chat_idle_timeout_minutes")]
    pub chat_idle_timeout_minutes: u32,
    #[serde(default)]
    pub planner_provider_id: String,
    #[serde(default)]
    pub planner_model: String,
    #[serde(default)]
    #[serde(alias = "orchestration")]
    pub mutis: MutisConfig,
    #[serde(default)]
    pub self_dev: SelfDevConfig,
}

fn default_chat_idle_timeout_minutes() -> u32 {
    10
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelfDevConfig {
    #[serde(default)]
    pub auto_rebuild: bool,
}

fn default_folder() -> String {
    r"C:\TOOL".to_owned()
}

impl Config {
    pub fn seeded() -> Self {
        Self {
            memory: Default::default(),
            providers: vec![
                ProviderConfig {
                    id: "deploy-deepseek".to_owned(),
                    name: "Deploy (DeepSeek)".to_owned(),
                    kind: "openai_compat".to_owned(),
                    base_url: "https://api.deploy.com/api/v1".to_owned(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["deepseek-ai/deepseek-v4.1-flash-1017".to_owned()],
                    agent_capable: true,
                },
                ProviderConfig {
                    id: "openai".to_owned(),
                    name: "OpenAI".to_owned(),
                    kind: "openai_compat".to_owned(),
                    base_url: "https://api.openai.com/v1".to_owned(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["gpt-5.1".to_owned()],
                    ..Default::default()
                },
                ProviderConfig {
                    id: "anthropic".to_owned(),
                    name: "Anthropic".to_owned(),
                    kind: "anthropic".to_owned(),
                    base_url: "https://api.anthropic.com".to_owned(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["claude-opus-5-5".to_owned(), "claude-sonnet-5-5".to_owned()],
                    ..Default::default()
                },
                ProviderConfig {
                    id: "codex-cli".to_owned(),
                    name: "Codex CLI".to_owned(),
                    kind: "codex_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec![
                        "gpt-6-luna".to_owned(),
                        "gpt-6.1-sol".to_owned(),
                        "gpt-6.1-terra".to_owned(),
                    ],
                    ..Default::default()
                },
                ProviderConfig {
                    id: "claude-cli".to_owned(),
                    name: "Claude Code CLI".to_owned(),
                    kind: "claude_cli".to_owned(),
                    base_url: String::new(),
                    api_key: String::new(),
                    executable: String::new(),
                    models: vec!["opus".to_owned(), "sonnet".to_owned()],
                    ..Default::default()
                },
            ],
            default_folder: default_folder(),
            chat_idle_timeout_minutes: default_chat_idle_timeout_minutes(),
            planner_provider_id: String::new(),
            planner_model: String::new(),
            mutis: MutisConfig::default(),
            self_dev: SelfDevConfig::default(),
        }
    }
}
