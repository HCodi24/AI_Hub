use super::*;

#[test]
fn chat_rule_depends_on_how_deepseek_can_be_reached() {
    assert_eq!(for_chat(true, "high", "codex_cli", true, None), RULE);
    // No way to reach the worker: no delegation and no plan marker, only the language rule.
    assert_eq!(for_chat(true, "high", "codex_cli", false, None), VIETNAMESE_RULE);
    assert!(!VIETNAMESE_RULE.contains("MUTIS_PLAN"));
    assert_eq!(for_chat(true, "high", "codex_cli", false, Some(Worker::DeepSeek)), PROTOCOL_RULE);
    assert_eq!(for_chat(true, "high", "codex_cli", false, Some(Worker::Other)), OTHER_PROTOCOL_RULE);
    assert_eq!(for_chat(true, "max", "codex_cli", false, Some(Worker::DeepSeek)), "");
    // The other-model rule keeps the protocol but drops DeepSeek's exact-edit demands.
    // Big and small work exists only with DeepSeek: another worker is capable and takes every task.
    assert!(OTHER_PROTOCOL_RULE.contains("```mutis-delegate") && OTHER_PROTOCOL_RULE.contains("\"trivial\":true"));
    assert!(!OTHER_PROTOCOL_RULE.contains("hard") && !OTHER_PROTOCOL_RULE.contains("second editor"));
    assert!(!OTHER_PROTOCOL_RULE.contains("DeepSeek") && OTHER_PROTOCOL_RULE.contains("does not need line ranges") && PROTOCOL_RULE.contains("exact files"));
    // Splitting the job is the coordinator's duty with any worker; DeepSeek only gets the finer split.
    for rule in [PROTOCOL_RULE, OTHER_PROTOCOL_RULE] {
        assert!(rule.contains("Always analyse the job and split it") && rule.contains("split into several tasks"));
    }
    assert!(PROTOCOL_RULE.contains("split finer") && !OTHER_PROTOCOL_RULE.contains("split finer"));
    assert!(OTHER_PROTOCOL_RULE.lines().count() >= 10, "escapes must stay \\n, not literal newlines inside strings");
    assert!(PROTOCOL_RULE.contains("```mutis-delegate") && PROTOCOL_RULE.contains("\"hard\":true") && PROTOCOL_RULE.contains("big task") && !PROTOCOL_RULE.contains("second editor") && PROTOCOL_RULE.contains("\"trivial\":true"));
    assert!(PROTOCOL_RULE.lines().count() >= 10, "escapes must stay \\n, not literal newlines inside strings");
}

#[test]
fn delegation_is_scoped_to_enabled_high_cli_leads() {
    assert_eq!(for_coordinator(true, "high", "codex_cli"), RULE);
    assert_eq!(for_coordinator(false, "high", "codex_cli"), "");
    assert_eq!(for_coordinator(true, "max", "codex_cli"), "");
    assert_eq!(for_coordinator(true, "high", "openai_compat"), "");
    assert_eq!(for_coordinator(true, "high", "claude_cli"), RULE, "Claude leads the same way Codex does");
}

#[test]
fn every_coordinator_rule_requires_vietnamese_visible_prose() {
    for rule in [RULE, PROTOCOL_RULE, OTHER_PROTOCOL_RULE] {
        assert!(rule.contains("user-facing word in Vietnamese"), "missing language rule: {rule}");
        assert!(rule.contains("even when the user's request or the task instructions are in English"));
        // The evidence the coordinator must keep verbatim is named, so the rule cannot be read as "translate everything".
        assert!(rule.contains("paths") && rule.contains("JSON keys") && rule.contains("literal tool or error output"));
    }
    // The shared constant is the same instruction the worker and the dev chat receive.
    assert!(VIETNAMESE_RULE.contains("Vietnamese") && VIETNAMESE_RULE.contains("exactly as they are"));
}
