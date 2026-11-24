use super::{for_chat, for_coordinator, protocol, VIETNAMESE_RULE};

#[test]
fn chat_uses_the_same_protocol_for_either_delegation_route() {
    assert_eq!(for_chat(true, true, false), protocol::RULE);
    assert_eq!(for_chat(true, false, true), protocol::RULE);
    assert_eq!(for_chat(true, true, true), protocol::RULE);
    assert_eq!(for_chat(true, false, false), VIETNAMESE_RULE);
    assert_eq!(for_chat(false, true, true), "");
    assert_eq!(for_coordinator(true), protocol::RULE);
    assert_eq!(for_coordinator(false), VIETNAMESE_RULE);
}

#[test]
fn protocol_keeps_the_split_and_delegate_block_contract() {
    for required in [
        "Always analyse the job and split it",
        "split into several tasks",
        "```mutis-delegate",
        "\"depends_on\"",
        "\"trivial\":true",
        "does not need line ranges",
    ] {
        assert!(protocol::RULE.contains(required), "{required}");
    }
    for removed in ["MutiS High", "DeepSeek", "\"hard\":true"] {
        assert!(!protocol::RULE.contains(removed), "{removed}");
    }
}

#[test]
fn both_rules_require_vietnamese_visible_prose_and_verbatim_evidence() {
    for rule in [protocol::RULE, VIETNAMESE_RULE] {
        assert!(rule.contains("user-facing word in Vietnamese"));
        assert!(rule.contains("paths"));
        assert!(rule.contains("JSON keys"));
        assert!(rule.contains("literal tool or error output"));
    }
}
