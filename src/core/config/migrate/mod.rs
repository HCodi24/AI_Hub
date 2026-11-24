//! One-way upgrades of config files written by older versions.
use super::{seed::CODEX_CLI_MODELS, Config};

mod roles;

const LEGACY_WORKER_RULES: &str = concat!(
    "- Ghi mọi file bằng UTF-8 (không dùng PowerShell Set-Content/Out-File/>); ",
    "dùng công cụ sửa file hoặc [System.IO.File]::WriteAllText với UTF8.\n",
    "- Không xoá tính năng có sẵn khi viết lại code.\n",
    "- Báo cáo rõ phần chưa làm."
);

pub(super) fn add_codex_cli_model_suggestions(config: &mut Config) {
    for provider in config
        .providers
        .iter_mut()
        .filter(|provider| provider.kind == "codex_cli")
    {
        for model in CODEX_CLI_MODELS {
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
            &mut config.mutis.high_roles.lead,
            &mut config.mutis.high_roles.big,
            &mut config.mutis.high_roles.small,
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
    let previous_english_rules = [
        crate::core::rule::utf8::RULE,
        crate::core::rule::keep_features::RULE,
        crate::core::rule::report_unfinished::RULE,
    ].join("\n");
    let previous_global_english_rule = format!(
        "{previous_english_rules}\n{}",
        crate::core::rule::english_rules::RULE
    );
    let saved_rules = config.mutis.worker_rules.replace("\r\n", "\n");
    if saved_rules.trim() == LEGACY_WORKER_RULES
        || saved_rules.trim() == previous_english_rules
        || saved_rules.trim() == previous_global_english_rule
    {
        config.mutis.worker_rules = crate::core::rule::worker_defaults();
    }
    roles::migrate_high_roles(config);
    roles::migrate_max_roles(config);
    for rule in &mut config.mutis.routing {
        if rule.role.is_empty() {
            rule.role = match rule.assignee_kind.as_str() {
                "deepseek_agent" => "small",
                "codex_cli" => "big",
                _ => "lead",
            }
            .into();
        }
    }
    let high_lead = &config.mutis.high_roles.lead;
    let lead = if high_lead.provider_id.trim().is_empty() {
        &config.mutis.roles.lead
    } else {
        high_lead
    };
    config.planner_provider_id = lead.provider_id.clone();
    config.planner_model = lead.model.clone();
    config.mutis.reviewer_provider_id = lead.provider_id.clone();
    config.mutis.reviewer_model = lead.model.clone();
}

#[cfg(test)]
mod tests;
