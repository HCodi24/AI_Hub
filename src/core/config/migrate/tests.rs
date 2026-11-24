use super::*;
use crate::core::config::{ProviderConfig, LEGACY_AGENT_PROVIDER_ID};

#[test]
fn planner_and_reviewer_follow_the_new_lead() {
    let mut config = Config::seeded();
    config.mutis.lead.provider_id = "codex-cli".into();
    config.mutis.lead.model = "gpt-6.1-sol".into();

    migrate_mutis(&mut config);

    assert_eq!(config.planner_provider_id, "codex-cli");
    assert_eq!(config.planner_model, "gpt-6.1-sol");
    assert_eq!(config.mutis.reviewer_provider_id, "codex-cli");
    assert_eq!(config.mutis.reviewer_model, "gpt-6.1-sol");
}

#[test]
fn empty_lead_clears_legacy_planner_and_reviewer_settings() {
    let mut config = Config::seeded();
    config.planner_provider_id = "codex-cli".into();
    config.planner_model = "gpt-6-luna".into();
    config.mutis.reviewer_provider_id = "claude-cli".into();
    config.mutis.reviewer_model = "opus".into();

    migrate_mutis(&mut config);

    assert!(config.planner_provider_id.is_empty());
    assert!(config.planner_model.is_empty());
    assert!(config.mutis.reviewer_provider_id.is_empty());
    assert!(config.mutis.reviewer_model.is_empty());
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
