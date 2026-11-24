use crate::core::config::{Config, MutisRole, MutisRoles};

// High roles: Codex leads and handles code; DeepSeek handles small tasks.
pub(super) fn migrate_high_roles(config: &mut Config) {
    if [
        &config.mutis.high_roles.lead,
        &config.mutis.high_roles.big,
        &config.mutis.high_roles.small,
    ]
    .iter()
    .all(|r| r.provider_id.is_empty())
    {
        let codex = config.providers.iter().find(|p| p.kind == "codex_cli");
        let deepseek = config
            .providers
            .iter()
            .find(|p| p.id == config.mutis.roles.small.provider_id)
            .or_else(|| {
                config.providers.iter().find(|p| {
                    [&p.id, &p.name]
                        .iter()
                        .any(|s| s.to_ascii_lowercase().contains("deepseek"))
                        || p.models
                            .iter()
                            .any(|s| s.to_ascii_lowercase().contains("deepseek"))
                })
            });
        if let Some(codex) = codex {
            let codex_role = || MutisRole {
                provider_id: codex.id.clone(),
                model: "gpt-6.1-sol".into(),
                effort: "medium".into(),
            };
            let small = deepseek
                .map(|p| MutisRole {
                    provider_id: p.id.clone(),
                    model: p.models.first().cloned().unwrap_or_default(),
                    effort: "max".into(),
                })
                .unwrap_or_default();
            config.mutis.high_roles = MutisRoles {
                lead: codex_role(),
                big: codex_role(),
                small,
            };
        }
    }
}

// Max roles: initialize and migrate the three-AI role profile.
pub(super) fn migrate_max_roles(config: &mut Config) {
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
    config.mutis.roles = roles;
}
