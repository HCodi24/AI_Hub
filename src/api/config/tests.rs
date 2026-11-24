use super::{apply, validate, ConfigInput};
use crate::config::Config;

#[test]
fn partial_update_only_carries_fields_that_were_sent() {
    let input: ConfigInput = serde_json::from_str(r#"{"default_folder":"C:/TOOL"}"#).unwrap();
    assert_eq!(input.default_folder.as_deref(), Some("C:/TOOL"));
    assert!(input.theme.is_none());
    assert!(input.planner_provider_id.is_none());
    assert!(input.planner_model.is_none());
    assert!(input.chat_idle_timeout_minutes.is_none());
    assert!(input.memory.is_none());
    let full: ConfigInput =
        serde_json::from_str(r#"{"default_folder":"x","chat_idle_timeout_minutes":0}"#)
            .unwrap();
    assert_eq!(full.chat_idle_timeout_minutes, Some(0));
}

#[test]
fn rejects_invalid_config_values() {
    for json in [
        r#"{"chat_idle_timeout_minutes":121}"#,
        r#"{"approval_wait_seconds":9}"#,
        r#"{"approval_wait_seconds":3601}"#,
        r#"{"thread_token_budget":19999}"#,
        r#"{"theme":"unknown"}"#,
        r#"{"pets":{"unknown":"valid-pet"}}"#,
        r#"{"pets":{"deepseek":"Invalid Pet"}}"#,
    ] {
        let input: ConfigInput = serde_json::from_str(json).unwrap();
        assert!(validate(&input).is_err(), "{json}");
    }
}

#[test]
fn accepts_valid_partial_config_and_token_budget_boundaries() {
    for json in [
        r#"{"default_folder":"C:/TOOL"}"#,
        r#"{"thread_token_budget":0}"#,
        r#"{"thread_token_budget":20000}"#,
    ] {
        let input: ConfigInput = serde_json::from_str(json).unwrap();
        assert!(validate(&input).is_ok(), "{json}");
    }
}

#[test]
fn applying_planner_provider_updates_lead_and_reviewer() {
    for (high_provider, expected) in [(" ", " "), ("old-high", "new-provider")] {
        let mut config = Config::seeded();
        config.mutis.high_roles.lead.provider_id = high_provider.into();
        let input = serde_json::from_str(r#"{"planner_provider_id":"new-provider"}"#).unwrap();

        apply(&mut config, input);

        assert_eq!(config.planner_provider_id, "new-provider");
        assert_eq!(config.mutis.roles.lead.provider_id, "new-provider");
        assert_eq!(config.mutis.reviewer_provider_id, "new-provider");
        assert_eq!(config.mutis.high_roles.lead.provider_id, expected);
    }
}

#[test]
fn applying_planner_model_updates_lead_and_reviewer() {
    for (high_provider, expected) in [(" ", "old-model"), ("high", "new-model")] {
        let mut config = Config::seeded();
        config.mutis.high_roles.lead.provider_id = high_provider.into();
        config.mutis.high_roles.lead.model = "old-model".into();
        let input = serde_json::from_str(r#"{"planner_model":"new-model"}"#).unwrap();

        apply(&mut config, input);

        assert_eq!(config.planner_model, "new-model");
        assert_eq!(config.mutis.roles.lead.model, "new-model");
        assert_eq!(config.mutis.reviewer_model, "new-model");
        assert_eq!(config.mutis.high_roles.lead.model, expected);
    }
}

#[test]
fn applying_mcp_servers_bumps_codex_revision() {
    let mut config = Config::seeded();
    config.codex.revision = 41;
    let input = serde_json::from_str(r#"{"mcp_servers":[]}"#).unwrap();

    apply(&mut config, input);

    assert_eq!(config.codex.revision, 42);
}

#[test]
fn applying_default_folder_trims_whitespace() {
    let mut config = Config::seeded();
    let input = serde_json::from_str(r#"{"default_folder":"  C:/TOOL  "}"#).unwrap();

    apply(&mut config, input);

    assert_eq!(config.default_folder, "C:/TOOL");
}
