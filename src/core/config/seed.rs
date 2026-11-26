use super::{
    default_approval_wait_seconds, default_chat_idle_timeout_minutes, default_folder,
    default_thread_token_budget, AntigravityConfig, ClaudeConfig, CodexConfig, Config, MutisConfig,
    ProviderConfig, SelfDevConfig,
};

#[cfg(test)]
mod tests;

pub(super) const CODEX_CLI_MODELS: [&str; 4] =
    ["gpt-6-luna", "gpt-6.1-sol", "gpt-6.2-sol", "gpt-6.1-terra"];

fn provider(id: &str, name: &str, kind: &str, base_url: &str, models: &[&str]) -> ProviderConfig {
    ProviderConfig {
        id: id.to_owned(),
        name: name.to_owned(),
        kind: kind.to_owned(),
        base_url: base_url.to_owned(),
        models: models.iter().copied().map(str::to_owned).collect(),
        ..Default::default()
    }
}

impl Config {
    pub fn seeded() -> Self {
        Self {
            claude: ClaudeConfig::default(),
            antigravity: AntigravityConfig::default(),
            codex: CodexConfig::default(),
            memory: Default::default(),
            providers: vec![
                ProviderConfig {
                    agent_capable: true,
                    ..provider(
                        "deploy-deepseek",
                        "Deploy (DeepSeek)",
                        "openai_compat",
                        "https://api.deploy.com/api/v1",
                        &["deepseek-ai/deepseek-v4.1-flash-1017"],
                    )
                },
                provider(
                    "openai",
                    "OpenAI",
                    "openai_compat",
                    "https://api.openai.com/v1",
                    &["gpt-5.1"],
                ),
                provider(
                    "anthropic",
                    "Anthropic",
                    "anthropic",
                    "https://api.anthropic.com",
                    &["claude-opus-5-5", "claude-sonnet-5-5"],
                ),
                provider("codex-cli", "Codex CLI", "codex_cli", "", &CODEX_CLI_MODELS),
                provider(
                    "claude-cli",
                    "Claude Code CLI",
                    "claude_cli",
                    "",
                    &["opus", "sonnet"],
                ),
                provider(
                    "antigravity-cli",
                    "Antigravity CLI",
                    "antigravity_cli",
                    "",
                    &[
                        "gemini-4-neon-high",
                        "gemini-4-carbon-high",
                        "gemini-4-argon-high",
                        "gemini-4-hydrogen-high",
                    ],
                ),
            ],
            default_folder: default_folder(),
            chat_idle_timeout_minutes: default_chat_idle_timeout_minutes(),
            thread_token_budget: default_thread_token_budget(),
            approval_wait_seconds: default_approval_wait_seconds(),
            planner_provider_id: String::new(),
            planner_model: String::new(),
            mutis: MutisConfig::default(),
            self_dev: SelfDevConfig::default(),
            theme: String::new(),
            style: String::new(),
            mcp_servers: Vec::new(),
            pets: Default::default(),
            pets_enabled: true,
            codegraph_enabled: true,
            claude_usage_api: true,
        }
    }
}
