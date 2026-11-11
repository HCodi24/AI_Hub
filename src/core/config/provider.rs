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

pub fn first_connected_fallback(
    providers: &[ProviderConfig],
    role: &str,
    connected: &std::collections::HashSet<String>,
) -> Option<String> {
    if role == "small" {
        // Agent-capable OpenAI-compatible source (e.g. Deploy/DeepSeek in agent mode) first, then CLIs.
        if let Some(provider) = providers.iter().find(|provider| {
            provider.kind == "openai_compat"
                && provider.agent_capable
                && connected.contains(&provider.id)
        }) {
            return Some(provider.id.clone());
        }
    }
    let kinds: &[&str] = match role {
        "lead" => &["claude_cli", "anthropic", "codex_cli"],
        "big" => &["codex_cli", "claude_cli"],
        "small" => &["codex_cli", "claude_cli"],
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

    #[test]
    fn removed_agent_id_still_resolves_to_the_agent_capable_source() {
        let config = Config::seeded();
        let found = resolve_provider_id(&config.providers, LEGACY_AGENT_PROVIDER_ID).unwrap();
        assert_eq!(found.id, "deploy-deepseek");
        assert!(resolve_provider_id(&config.providers, "nope").is_none());
    }

    #[test]
    fn small_role_falls_back_from_the_agent_capable_source_to_codex() {
        let config = Config::seeded();
        let mut connected = std::collections::HashSet::new();
        connected.insert("codex-cli".to_owned());
        assert_eq!(
            first_connected_fallback(&config.providers, "small", &connected).as_deref(),
            Some("codex-cli")
        );
        connected.insert("deploy-deepseek".to_owned());
        assert_eq!(
            first_connected_fallback(&config.providers, "small", &connected).as_deref(),
            Some("deploy-deepseek")
        );
    }
}
