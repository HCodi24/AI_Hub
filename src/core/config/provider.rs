use serde::{Deserialize, Serialize};


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
    /// OpenAI-compatible sources only: the AI may use file tools (list/read/edit/write, never commands) inside
    /// the chat folder through this source ("agent mode"). Replaces the old separate `deepseek_agent` source.
    #[serde(default)]
    pub agent_capable: bool,
    /// CLI sources only: this source signs in through the web (Settings › Tài khoản web) instead of reusing
    /// the account of the CLI installed on this machine.
    #[serde(default)]
    pub web_login: bool,
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
            && [model, self.id.as_str(), self.name.as_str()].iter().any(|text| text.to_ascii_lowercase().contains("deepseek"))
    }
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

#[allow(clippy::items_after_test_module)]
pub fn is_masked_key(value: &str) -> bool {
    value.contains('…') || value.contains("••") || value.starts_with("****")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Config;

    #[test]
    fn masked_key_never_contains_raw_secret() {
        let raw = "sk-live-secret-value";
        let masked = masked_key(raw);
        assert_ne!(masked, raw);
        assert!(!masked.contains("secret-value"));
        assert!(masked.ends_with("alue"));
    }

    #[test]
    fn removed_agent_id_still_resolves_to_the_agent_capable_source() {
        let config = Config::seeded();
        let found = resolve_provider_id(&config.providers, LEGACY_AGENT_PROVIDER_ID).unwrap();
        assert_eq!(found.id, "deploy-deepseek");
        assert!(resolve_provider_id(&config.providers, "nope").is_none());
    }
}
