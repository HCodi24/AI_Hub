use super::*;

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

fn assert_new_role_defaults(config: &MutisConfig) {
    let json = serde_json::to_value(config).unwrap();
    assert_eq!(
        json["lead"],
        serde_json::json!({"provider_id":"","model":"","effort":"default"}),
    );
    assert_eq!(
        json["worker"],
        serde_json::json!({
            "provider_id":"","model":"","effort":"default","permission":"full","count":1
        }),
    );
    assert_eq!(
        json["checker"],
        serde_json::json!({
            "provider_id":"","model":"","effort":"default","permission":"full"
        }),
    );
}

#[test]
fn new_roles_default_to_unconfigured() {
    assert_new_role_defaults(&MutisConfig::default());
    let partial: MutisConfig =
        serde_json::from_str(r#"{"lead":{},"worker":{},"checker":{}}"#).unwrap();
    assert_new_role_defaults(&partial);
}

#[test]
fn legacy_config_loads_and_drops_retired_keys() {
    let old_roles = serde_json::json!({
        "lead": {"provider_id":"old-lead","model":"old-model","effort":"high"},
        "big": {},
        "small": {}
    });
    let retired = [
        ("roles", old_roles.clone()),
        (concat!("high_", "roles"), old_roles),
        (
            concat!("high_", "checker"),
            serde_json::json!({"provider_id":"old-checker","effort":"max"}),
        ),
        ("routing", serde_json::json!([{"kind":"code","role":"big"}])),
        (concat!("deepseek_", "ui"), serde_json::json!(true)),
    ];
    let mut mutis = serde_json::json!({
        "reviewer_provider_id": "reviewer",
        "reviewer_model": "reviewer-model"
    });
    for (key, value) in &retired {
        mutis[*key] = value.clone();
    }
    let legacy: super::super::Config =
        serde_json::from_value(serde_json::json!({"mutis": mutis})).unwrap();
    assert_new_role_defaults(&legacy.mutis);
    let saved = serde_json::to_value(legacy).unwrap();
    for (key, _) in retired {
        assert!(saved["mutis"].get(key).is_none(), "{key}");
    }
    assert_eq!(saved["mutis"]["reviewer_provider_id"], "reviewer");
    assert_eq!(saved["mutis"]["reviewer_model"], "reviewer-model");
}

#[test]
fn new_roles_round_trip_with_explicit_settings() {
    let input = serde_json::json!({
        "lead": {"provider_id":"lead","model":"lead-model","effort":"high"},
        "worker": {
            "provider_id":"worker","model":"worker-model","effort":"medium",
            "permission":"write","count":3
        },
        "checker": {
            "provider_id":"checker","model":"checker-model","effort":"low","permission":"read"
        }
    });
    let decoded: MutisConfig = serde_json::from_value(input.clone()).unwrap();
    let encoded = serde_json::to_value(decoded).unwrap();
    for key in ["lead", "worker", "checker"] {
        assert_eq!(encoded[key], input[key]);
    }
}

#[test]
fn new_worker_and_checker_reject_unknown_permissions() {
    for role in ["worker", "checker"] {
        let input = format!(r#"{{"{role}":{{"permission":"admin"}}}}"#);
        assert!(serde_json::from_str::<MutisConfig>(&input).is_err());
    }
}
