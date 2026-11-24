use super::{MutisRole, RoutingRule};

pub(super) fn default_high_checker() -> MutisRole {
    MutisRole {
        effort: "max".into(),
        ..Default::default()
    }
}

pub(super) fn default_plan_approval() -> bool {
    false
}

pub(super) fn default_worker_rules() -> String {
    crate::core::rule::worker_defaults()
}

pub(super) fn default_deepseek_max_steps() -> u32 {
    15
}

pub(super) fn default_debate_rounds() -> u32 {
    2
}

pub(super) fn default_retry_attempts() -> u32 {
    3
}

pub(super) fn default_retry_base_ms() -> u64 {
    2000
}

pub(super) fn default_watchdog_minutes() -> u32 {
    5
}

pub(super) fn default_resume_max() -> u32 {
    2
}

pub(super) fn default_max_file_lines() -> u32 {
    400
}

pub(super) fn default_fix_rounds() -> u32 {
    2
}

pub(super) fn default_event_log() -> bool {
    true
}

/// MutiS is always on; this keeps `enabled` serialized as `true` and ignores incoming values.
pub(super) fn default_enabled() -> bool {
    true
}

pub(super) fn default_verify_ignore() -> Vec<String> {
    vec!["docs/**".into(), "**/*.md".into()]
}

pub(super) fn default_routing() -> Vec<RoutingRule> {
    [
        ("small_edit", "small"),
        ("docs", "small"),
        ("code", "big"),
        ("test", "big"),
        ("plan", "lead"),
        ("review", "lead"),
    ]
    .into_iter()
    .map(|(kind, role)| RoutingRule {
        kind: kind.into(),
        role: role.into(),
        assignee_kind: String::new(),
        note: String::new(),
    })
    .collect()
}

pub(super) fn default_effort() -> String {
    "default".into()
}
