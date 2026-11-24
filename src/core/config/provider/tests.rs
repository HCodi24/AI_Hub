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
fn removed_agent_id_still_resolves_to_the_agent_capable_source() {
    let config = Config::seeded();
    let found = resolve_provider_id(&config.providers, LEGACY_AGENT_PROVIDER_ID).unwrap();
    assert_eq!(found.id, "deploy-deepseek");
    assert!(resolve_provider_id(&config.providers, "nope").is_none());
}
