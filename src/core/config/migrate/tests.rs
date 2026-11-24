use super::*;
use crate::core::config::{MutisRoles, ProviderConfig, LEGACY_AGENT_PROVIDER_ID};

#[test]
fn planner_and_reviewer_follow_the_high_lead() {
    let mut config = Config::seeded();
    config.mutis.high_roles.lead.provider_id = "codex-cli".into();
    config.mutis.high_roles.lead.model = "gpt-6.1-sol".into();
    config.mutis.roles.lead.provider_id = "claude-cli".into();
    config.mutis.roles.lead.model = "opus".into();

    migrate_mutis(&mut config);

    assert_eq!(config.planner_provider_id, "codex-cli");
    assert_eq!(config.planner_model, "gpt-6.1-sol");
    assert_eq!(config.mutis.reviewer_provider_id, "codex-cli");
    assert_eq!(config.mutis.reviewer_model, "gpt-6.1-sol");
}

#[test]
fn planner_and_reviewer_fall_back_when_the_high_lead_provider_is_blank() {
    let mut config = Config::seeded();
    config.mutis.high_roles.lead.provider_id = " \t".into();
    config.mutis.high_roles.lead.model = "gpt-6.1-sol".into();
    config.mutis.roles.lead.provider_id = "claude-cli".into();
    config.mutis.roles.lead.model = "opus".into();

    migrate_mutis(&mut config);

    assert_eq!(config.planner_provider_id, "claude-cli");
    assert_eq!(config.planner_model, "opus");
    assert_eq!(config.mutis.reviewer_provider_id, "claude-cli");
    assert_eq!(config.mutis.reviewer_model, "opus");
}

#[test]
fn merging_an_agent_provider_remaps_all_high_roles() {
    let mut config = legacy_config("sk-agent-key");
    let high_roles = &mut config.mutis.high_roles;
    for role in [&mut high_roles.lead, &mut high_roles.big, &mut high_roles.small] {
        role.provider_id = LEGACY_AGENT_PROVIDER_ID.into();
    }

    merge_agent_providers(&mut config);

    let high_roles = &config.mutis.high_roles;
    for role in [&high_roles.lead, &high_roles.big, &high_roles.small] {
        assert_eq!(role.provider_id, "deploy-deepseek");
    }
}

#[test]
fn migration_translates_default_rules_and_preserves_custom_rules() {
    let mut config = Config::seeded();
    config.mutis.worker_rules = LEGACY_WORKER_RULES.replace("\n", "\r\n");
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.worker_rules, crate::core::rule::worker_defaults());
    let translated = config.mutis.worker_rules.clone();
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.worker_rules, translated);

    config.mutis.worker_rules = [
        crate::core::rule::utf8::RULE,
        crate::core::rule::keep_features::RULE,
        crate::core::rule::report_unfinished::RULE,
    ].join("\r\n");
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.worker_rules, crate::core::rule::worker_defaults());
    assert!(!config.mutis.worker_rules.contains("- Write all rules in English."));

    config.mutis.worker_rules = format!("{}\n- Write all rules in English.", [
        crate::core::rule::utf8::RULE,
        crate::core::rule::keep_features::RULE,
        crate::core::rule::report_unfinished::RULE,
    ].join("\n"));
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.worker_rules, crate::core::rule::worker_defaults());

    config.mutis.worker_rules = format!("{LEGACY_WORKER_RULES}\nCustom instruction");
    let custom = config.mutis.worker_rules.clone();
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.worker_rules, custom);
}

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

#[test]
fn migration_populates_high_roles_with_codex_and_existing_small_deepseek() {
    let mut config = Config::seeded();
    config.mutis.roles.small.provider_id = "deploy-deepseek".into();
    migrate_mutis(&mut config);
    assert_eq!(config.mutis.high_roles.lead.provider_id, "codex-cli");
    assert_eq!(config.mutis.high_roles.lead.model, "gpt-6.1-sol");
    assert_eq!(config.mutis.high_roles.big.effort, "medium");
    assert_eq!(config.mutis.high_roles.small.provider_id, "deploy-deepseek");
    assert_eq!(config.mutis.high_roles.small.effort, "max");
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
