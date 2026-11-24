mod mcp;
mod codex;
mod claude;
mod migrate;
mod mutis;
mod provider;
mod seed;
mod store;

pub use mcp::*;
pub use codex::*;
pub use claude::*;
pub use mutis::*;
pub use provider::*;
pub use store::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AntigravityConfig {
    pub project_skills: bool,
    pub project_instructions: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub antigravity: AntigravityConfig,
    #[serde(default)]
    pub claude: ClaudeConfig,
    #[serde(default)]
    pub codex: CodexConfig,
    #[serde(default)]
    pub memory: crate::features::memory::MemoryConfig,
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
    #[serde(default = "default_folder")]
    pub default_folder: String,
    #[serde(default = "default_chat_idle_timeout_minutes")]
    pub chat_idle_timeout_minutes: u32,
    /// A Codex thread whose last model call read this many input tokens is compacted (Codex summarises its own history) before
    /// its next turn. Only at twice this size, if compacting did not help, is the thread replaced by a new one on the next message.
    /// 0 = never.
    #[serde(default = "default_thread_token_budget")]
    pub thread_token_budget: u64,
    /// Seconds a run waits for the user to allow or refuse a command it paused on; no answer counts as "no".
    #[serde(default = "default_approval_wait_seconds")]
    pub approval_wait_seconds: u32,
    #[serde(default)]
    pub planner_provider_id: String,
    #[serde(default)]
    pub planner_model: String,
    #[serde(default)]
    #[serde(alias = "orchestration")]
    pub mutis: MutisConfig,
    #[serde(default)]
    pub self_dev: SelfDevConfig,
    /// Interface theme: "light", "dark" or "system"; empty until the user picks one.
    #[serde(default)]
    pub theme: String,
    /// MCP servers MutiS runs for in-app (API source) agents.
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
    /// Chosen chat avatar pet per slot (`codex-luna`, `claude-opus`, ..., `deepseek`; API and CLI share one); missing = no pet.
    #[serde(default)]
    pub pets: std::collections::BTreeMap<String, String>,
    /// Master switch for the chat avatar pets.
    #[serde(default = "default_true")]
    pub pets_enabled: bool,
    /// Master switch for CodeGraph: before a chat's first message the app builds or syncs the index and hands the AI its context.
    #[serde(default = "default_true")]
    pub codegraph_enabled: bool,
    /// Claude's usage limits are also asked from Anthropic (`/api/oauth/usage`, with Claude Code's own login) so they show before the first
    /// chat; off = only the limit events the CLI prints during a run.
    #[serde(default = "default_true")]
    pub claude_usage_api: bool,
}

fn default_true() -> bool {
    true
}

fn default_chat_idle_timeout_minutes() -> u32 {
    10
}

pub(crate) fn default_approval_wait_seconds() -> u32 {
    120
}

fn default_thread_token_budget() -> u64 {
    150_000
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SelfDevConfig {
    #[serde(default)]
    pub auto_rebuild: bool,
}

fn default_folder() -> String {
    String::new()
}
