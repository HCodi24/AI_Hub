use super::*;

#[test]
fn every_effort_uses_the_high_roles() {
    let mut config = MutisConfig::default();
    config.roles.lead.provider_id = "max".into();
    config.high_roles.lead.provider_id = "high".into();
    for effort in ["default", "medium", "high", "max", "xhigh"] { assert_eq!(config.roles_for(effort).lead.provider_id, "high"); }
    assert_eq!(MutisConfig::level_for("xhigh"), "high");
    assert_eq!(MutisConfig::level_for("medium"), "high");
}

#[test]
fn the_checker_role_defaults_to_max_effort_and_old_configs_get_it() {
    assert_eq!(MutisConfig::default().high_checker.effort, "max");
    let old: MutisConfig = serde_json::from_str(r#"{"enabled":true}"#).unwrap();
    assert!(old.high_checker.provider_id.is_empty() && old.high_checker.effort == "max");
    let set: MutisConfig = serde_json::from_str(r#"{"enabled":true,"high_checker":{"provider_id":"codex-cli","model":"gpt-6-luna","effort":"xhigh"}}"#).unwrap();
    assert_eq!((set.high_checker.provider_id.as_str(), set.high_checker.effort.as_str()), ("codex-cli", "xhigh"));
}

#[test]
fn mutis_is_always_enabled_and_keeps_the_serialized_field() {
    // Default configuration is enabled.
    assert!(MutisConfig::default().enabled);
    // Omitted `enabled` becomes true.
    let omitted: MutisConfig = serde_json::from_str("{}").unwrap();
    assert!(omitted.enabled);
    // Legacy `enabled:false` is ignored and unrelated flags keep their values.
    let legacy: MutisConfig = serde_json::from_str(r#"{"enabled":false,"auto_plan":true,"event_log":false,"retry_attempts":5}"#).unwrap();
    assert!(legacy.enabled);
    assert!(legacy.auto_plan);
    assert!(!legacy.event_log);
    assert_eq!(legacy.retry_attempts, 5);
    // `enabled:true` stays true.
    let explicit: MutisConfig = serde_json::from_str(r#"{"enabled":true,"auto_fix":true}"#).unwrap();
    assert!(explicit.enabled);
    assert!(explicit.auto_fix);
    // The field is still serialized, always as true.
    let json = serde_json::to_value(MutisConfig::default()).unwrap();
    assert_eq!(json.get("enabled"), Some(&serde_json::Value::Bool(true)));
    let round_trip: MutisConfig = serde_json::from_value(json).unwrap();
    assert!(round_trip.enabled);
}
