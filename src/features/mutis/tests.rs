use super::{apply_setting_change, normalize_error, parse_diagnosis, validate_diagnosis};
use serde_json::{json, Value};

#[test]
fn parses_diagnosis_json_and_normalizes_numbered_errors() {
    let parsed = parse_diagnosis(
        "```json\n{\"health\":\"tốt\",\"findings\":[],\"summary\":\"Ổn\",\"extra\":true}\n```",
    )
    .unwrap();
    assert_eq!(parsed["health"], "tốt");
    assert_eq!(
        normalize_error("HTTP 503 on attempt 12"),
        "HTTP # on attempt #"
    );
}

#[test]
fn validates_finding_shape_and_drops_unknown_keys() {
    let valid = json!({"health":"kém","summary":"Sự cố","extra":1,"findings":[{"problem":"p","evidence":"e","cause":"c","severity":"high","suggestion":"s","setting_change":null,"extra":true}]});
    let parsed = validate_diagnosis(valid).unwrap();
    assert!(parsed.get("extra").is_none());
    assert!(parsed["findings"][0].get("extra").is_none());
    assert!(
        validate_diagnosis(json!({"health":"unknown","summary":"x","findings":[]})).is_err()
    );
    assert!(validate_diagnosis(json!({"health":"tốt","summary":3,"findings":[]})).is_err());
}

#[test]
fn setting_change_allowlist_accepts_and_rejects_required_paths() {
    let mut config = crate::config::Config::seeded();
    let (old, new) =
        apply_setting_change(&mut config, "mutis.watchdog_minutes", &json!(8)).unwrap();
    assert_eq!(old, json!(5));
    assert_eq!(new, json!(8));
    apply_setting_change(&mut config, "mutis.roles.big.effort", &json!("high")).unwrap();
    apply_setting_change(&mut config, "mutis.routing[2].role", &json!("small")).unwrap();
    for path in [
        "mutis.roles.lead.api_key",
        "providers[0].api_key",
        "mutis.unknown",
        "mutis.verify_command",
        "mutis.worker_rules",
        "mutis.paths",
    ] {
        assert!(
            apply_setting_change(&mut config, path, &json!("x")).is_err(),
            "{path}"
        );
    }
    assert!(apply_setting_change(&mut config, "mutis.watchdog_minutes", &json!(61)).is_err());
    assert!(apply_setting_change(&mut config, "mutis.retry_attempts", &json!(0)).is_err());
    assert!(apply_setting_change(
        &mut config,
        "mutis.roles.big.provider_id",
        &json!("missing")
    )
    .is_err());
    assert!(apply_setting_change(&mut config, "mutis.routing[2].role", &Value::Null).is_err());
}
