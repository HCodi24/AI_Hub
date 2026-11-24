use super::cli::last_chars;
use super::codex::codex_default_plugin;
use serde_json::json;

#[test]
fn default_codex_plugins_are_detected() {
    assert!(codex_default_plugin(&json!({"pluginId": "sites@openai-curated-remote"})));
    assert!(codex_default_plugin(&json!({"pluginId": "x@market", "installedByDefault": true})));
    assert!(!codex_default_plugin(&json!({"pluginId": "x@openai-curated"})));
    assert!(!codex_default_plugin(&json!({"pluginId": "x@my-market"})));
}

#[test]
fn last_chars_keeps_the_unicode_suffix() {
    assert_eq!(last_chars("abế🙂", 2), "ế🙂");
    assert_eq!(last_chars("ế🙂", 4), "ế🙂");
    assert_eq!(last_chars("ế🙂", 0), "");
}
