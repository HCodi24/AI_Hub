use crate::config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigInput {
    pub(super) antigravity: Option<AntigravityInput>,
    #[serde(default)]
    pub(super) default_folder: Option<String>,
    #[serde(default)]
    pub(super) theme: Option<String>,
    #[serde(default)]
    pub(super) style: Option<String>,
    /// The whole MCP list when sent; omitted = unchanged.
    #[serde(default)]
    pub(super) mcp_servers: Option<Vec<config::McpServerConfig>>,
    /// The whole pet choice map when sent; omitted = unchanged.
    #[serde(default)]
    pub(super) pets: Option<std::collections::BTreeMap<String, String>>,
    #[serde(default)]
    pub(super) pets_enabled: Option<bool>,
    #[serde(default)]
    pub(super) codegraph_enabled: Option<bool>,
    #[serde(default)]
    pub(super) claude_usage_api: Option<bool>,
    #[serde(default)]
    pub(super) memory: Option<crate::features::memory::MemoryConfig>,
    // Optional on purpose: a partial update (e.g. the UI saving only the working folder) must NOT reset
    // settings it did not send. Only fields that are present are applied.
    #[serde(default)]
    pub(super) planner_provider_id: Option<String>,
    #[serde(default)]
    pub(super) planner_model: Option<String>,
    #[serde(default)]
    pub(super) chat_idle_timeout_minutes: Option<u32>,
    #[serde(default)]
    pub(super) thread_token_budget: Option<u64>,
    #[serde(default)]
    pub(super) approval_wait_seconds: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(super) struct AntigravityInput {
    pub(super) project_skills: Option<bool>,
    pub(super) project_instructions: Option<bool>,
}
