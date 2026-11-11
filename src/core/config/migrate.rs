//! One-way upgrades of config files written by older versions.
use super::Config;

pub(super) fn add_codex_cli_model_suggestions(config: &mut Config) {
    for provider in config
        .providers
        .iter_mut()
        .filter(|provider| provider.kind == "codex_cli")
    {
        for model in ["gpt-6-luna", "gpt-6.1-sol", "gpt-6.1-terra"] {
            if !provider.models.iter().any(|existing| existing == model) {
                provider.models.push(model.to_owned());
            }
        }
    }
}

fn same_endpoint(a: &str, b: &str) -> bool {
    a.trim()
        .trim_end_matches('/')
        .eq_ignore_ascii_case(b.trim().trim_end_matches('/'))
}

/// Merge every legacy `deepseek_agent` source into the OpenAI-compatible source of the same account:
/// the parent becomes `agent_capable` (agent mode), references are remapped, the duplicate is removed.
/// Without a matching parent the agent source is converted in place. Idempotent.
pub(super) fn merge_agent_providers(config: &mut Config) {
    let agent_ids: Vec<String> = config
        .providers
        .iter()
        .filter(|provider| provider.kind == "deepseek_agent")
        .map(|provider| provider.id.clone())
        .collect();
    for agent_id in agent_ids {
        let Some(agent) = config.providers.iter().find(|p| p.id == agent_id).cloned() else {
            continue;
        };
        let parent_index = config
            .providers
            .iter()
            .position(|p| {
                p.kind == "openai_compat"
                    && p.id == "deploy-deepseek"
                    && same_endpoint(&p.base_url, &agent.base_url)
            })
            .or_else(|| {
                config.providers.iter().position(|p| {
                    p.kind == "openai_compat" && same_endpoint(&p.base_url, &agent.base_url)
                })
            });
        let Some(parent_index) = parent_index else {
            if let Some(own) = config.providers.iter_mut().find(|p| p.id == agent_id) {
                own.kind = "openai_compat".into();
                own.agent_capable = true;
            }
            continue;
        };
        let parent_id = config.providers[parent_index].id.clone();
        {
            let parent = &mut config.providers[parent_index];
            parent.agent_capable = true;
            if parent.api_key.trim().is_empty() && !agent.api_key.trim().is_empty() {
                parent.api_key = agent.api_key.clone();
            }
            for model in &agent.models {
                if !parent.models.contains(model) {
                    parent.models.push(model.clone());
                }
            }
        }
        for role in [
            &mut config.mutis.roles.lead,
            &mut config.mutis.roles.big,
            &mut config.mutis.roles.small,
        ] {
            if role.provider_id == agent_id {
                role.provider_id = parent_id.clone();
            }
        }
        if config.planner_provider_id == agent_id {
            config.planner_provider_id = parent_id.clone();
        }
        if config.mutis.reviewer_provider_id == agent_id {
            config.mutis.reviewer_provider_id = parent_id.clone();
        }
        config.providers.retain(|p| p.id != agent_id);
    }
}

pub(super) fn migrate_mutis(config: &mut Config) {
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
            .find(|p| p.kind == "openai_compat" && p.agent_capable)
            .map(|p| p.id.clone())
            .unwrap_or_else(|| find(&["codex_cli"]));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{MutisRoles, ProviderConfig, LEGACY_AGENT_PROVIDER_ID};

    #[test]
    fn legacy_planner_reviewer_and_routing_migrate_to_roles() {
        let mut config = Config::seeded();
        config.planner_provider_id = "codex-cli".into();
        config.planner_model = "gpt-6-luna".into();
        config.mutis.reviewer_provider_id = "claude-cli".into();
        config.mutis.reviewer_model = "opus".into();
        config.mutis.routing[0].role.clear();
        config.mutis.routing[0].assignee_kind = "deepseek_agent".into(); // legacy value
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
    fn migration_fills_small_with_agent_capable_source_and_first_model() {
        let mut config = Config::seeded();
        config.mutis.roles.small.provider_id.clear();
        config.mutis.roles.small.model.clear();
        migrate_mutis(&mut config);
        assert_eq!(config.mutis.roles.small.provider_id, "deploy-deepseek");
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

    fn legacy_config(agent_key: &str) -> Config {
        let mut config = Config::seeded();
        config.providers.push(ProviderConfig {
            id: LEGACY_AGENT_PROVIDER_ID.into(),
            name: "DeepSeek làm việc (Deploy)".into(),
            kind: "deepseek_agent".into(),
            base_url: "https://api.deploy.com/api/v1/".into(),
            api_key: agent_key.into(),
            models: vec!["deepseek-ai/extra-model".into()],
            ..Default::default()
        });
        for provider in &mut config.providers {
            if provider.id == "deploy-deepseek" {
                provider.agent_capable = false;
            }
        }
        config.mutis.roles.small.provider_id = LEGACY_AGENT_PROVIDER_ID.into();
        config
    }

    #[test]
    fn legacy_agent_source_is_merged_into_the_matching_deploy_source() {
        let mut config = legacy_config("sk-agent-key");
        merge_agent_providers(&mut config);
        assert!(config.providers.iter().all(|p| p.kind != "deepseek_agent"));
        let deploy = config
            .providers
            .iter()
            .find(|p| p.id == "deploy-deepseek")
            .unwrap();
        assert!(deploy.agent_capable);
        assert_eq!(
            deploy.api_key, "sk-agent-key",
            "key is kept when only the agent had one"
        );
        assert!(deploy
            .models
            .contains(&"deepseek-ai/extra-model".to_owned()));
        assert_eq!(config.mutis.roles.small.provider_id, "deploy-deepseek");
    }

    #[test]
    fn merge_does_not_overwrite_an_existing_parent_key_and_is_idempotent() {
        let mut config = legacy_config("sk-agent-key");
        config
            .providers
            .iter_mut()
            .find(|p| p.id == "deploy-deepseek")
            .unwrap()
            .api_key = "sk-parent".into();
        merge_agent_providers(&mut config);
        let once = serde_json::to_string(&config).unwrap();
        merge_agent_providers(&mut config);
        assert_eq!(once, serde_json::to_string(&config).unwrap());
        let deploy = config
            .providers
            .iter()
            .find(|p| p.id == "deploy-deepseek")
            .unwrap();
        assert_eq!(deploy.api_key, "sk-parent");
    }

    #[test]
    fn legacy_agent_without_a_matching_parent_is_converted_in_place() {
        let mut config = legacy_config("sk-own");
        config
            .providers
            .retain(|p| p.id != "deploy-deepseek" && p.id != "openai");
        config
            .providers
            .iter_mut()
            .find(|p| p.id == LEGACY_AGENT_PROVIDER_ID)
            .unwrap()
            .base_url = "https://other.example/v1".into();
        merge_agent_providers(&mut config);
        let own = config
            .providers
            .iter()
            .find(|p| p.id == LEGACY_AGENT_PROVIDER_ID)
            .unwrap();
        assert_eq!(own.kind, "openai_compat");
        assert!(own.agent_capable);
        assert_eq!(own.api_key, "sk-own");
    }
}
