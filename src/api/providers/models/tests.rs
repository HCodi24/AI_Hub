use super::{catalog::ModelCatalog, openai::parse_model_ids};
use serde_json::json;

#[test]
fn codex_model_efforts_are_saved_by_model_id() {
    let mut catalog = ModelCatalog::default();
    catalog.add_codex(&json!({ "id": "gpt-6.2-sol" }));
    catalog.add_codex(&json!({
        "id": "gpt-6.2-sol",
        "supportedReasoningEfforts": [
            { "reasoningEffort": "medium" },
            { "reasoningEffort": "max" },
            { "reasoningEffort": "max" }
        ],
        "defaultReasoningEffort": "medium"
    }));
    catalog.add_codex(&json!({ "model": "legacy" }));
    catalog.add_codex(&json!({ "id": "hidden", "hidden": true, "defaultReasoningEffort": "high" }));

    assert_eq!(catalog.ids, ["gpt-6.2-sol", "legacy"]);
    assert_eq!(serde_json::to_value(&catalog.efforts).unwrap(), json!({
        "gpt-6.2-sol": { "supported": ["medium", "max"], "default": "medium" }
    }));
}

#[test]
fn claude_model_efforts_follow_the_resolved_id() {
    let mut catalog = ModelCatalog::default();
    catalog.add_claude(&json!({ "value": "claude-opus-5[1m]" }));
    catalog.add_claude(&json!({
        "value": "opus[1m]", "resolvedModel": "claude-opus-5[1m]",
        "supportsEffort": true, "supportedEffortLevels": ["low", "max", "max"]
    }));
    catalog.add_claude(&json!({ "value": "haiku" }));
    catalog.add_claude(&json!({ "value": "disabled", "disabled": true }));
    assert_eq!(catalog.ids, ["claude-opus-5[1m]", "haiku"]);
    assert_eq!(catalog.efforts.len(), 1);
    let effort = &catalog.efforts["claude-opus-5[1m]"];
    assert_eq!(effort.supported, ["low", "max"]);
    assert_eq!(effort.default, None);
}

#[test]
fn openai_model_ids_are_sorted_and_deduplicated() {
    let body = json!({
        "data": [{"id": "z"}, {"id": "a"}, {"id": "z"}, {"id": 7}, {}]
    });
    assert_eq!(parse_model_ids(&body), ["a", "z"]);
}
