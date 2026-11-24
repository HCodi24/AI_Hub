use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

mod secret;

pub use secret::{is_masked_key, masked_key};

#[cfg(test)]
mod tests;

#[derive(Default, Clone, Debug, Serialize, Deserialize)]
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
    /// Effort metadata reported by Codex App Server, keyed by model id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub model_efforts: BTreeMap<String, ModelEffort>,
    /// OpenAI-compatible sources only: the AI may use file tools (list/read/edit/write, never commands) inside
    /// the chat folder through this source ("agent mode"). Replaces the old separate `deepseek_agent` source.
    #[serde(default)]
    pub agent_capable: bool,
    /// CLI sources only: this source signs in through the web (Settings › Tài khoản web) instead of reusing
    /// the account of the CLI installed on this machine.
    #[serde(default)]
    pub web_login: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModelEffort {
    pub supported: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Id of the removed separate "DeepSeek làm việc" source; old chats/plans/roles may still mention it.
pub const LEGACY_AGENT_PROVIDER_ID: &str = "deepseek-agent-deploy";

/// Resolve a provider id, mapping the removed legacy agent id to the Deploy source.
pub fn resolve_provider_id<'a>(
    providers: &'a [ProviderConfig],
    id: &str,
) -> Option<&'a ProviderConfig> {
    providers.iter().find(|p| p.id == id).or_else(|| {
        (id == LEGACY_AGENT_PROVIDER_ID)
            .then(|| {
                providers
                    .iter()
                    .find(|p| p.kind == "openai_compat" && p.agent_capable)
            })
            .flatten()
    })
}

impl ProviderConfig {
    /// Whether this source runs a DeepSeek model: an API source whose model, id or name says so (a CLI never counts).
    pub fn is_deepseek(&self, model: &str) -> bool {
        !self.kind.ends_with("_cli")
            && [model, self.id.as_str(), self.name.as_str()]
                .iter()
                .any(|text| text.to_ascii_lowercase().contains("deepseek"))
    }
}

