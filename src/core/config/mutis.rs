use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutisConfig {
    #[serde(default = "default_worker_rules")]
    pub worker_rules: String,
    /// MutiS is always enabled. The field is kept (and serialized as `true`) for compatibility with
    /// persisted configurations and the web client, but incoming values are ignored.
    #[serde(default = "default_enabled", skip_deserializing)]
    pub enabled: bool,
    #[serde(default)]
    pub retry_api: bool,
    #[serde(default = "default_retry_attempts")]
    pub retry_attempts: u32,
    #[serde(default = "default_retry_base_ms")]
    pub retry_base_ms: u64,
    #[serde(default)]
    pub codex_watchdog: bool,
    #[serde(default = "default_watchdog_minutes")]
    pub watchdog_minutes: u32,
    #[serde(default)]
    pub codex_auto_resume: bool,
    #[serde(default = "default_resume_max")]
    pub resume_max: u32,
    #[serde(default)]
    pub deepseek_agent: bool,
    #[serde(default = "default_max_file_lines")]
    pub deepseek_max_file_lines: u32,
    #[serde(default = "default_deepseek_max_steps")]
    pub deepseek_max_steps: u32,
    /// "⚖ Tranh luận": maximum rounds (1 = independent proposals only, 2 = + critique, 3 = + final stance).
    #[serde(default = "default_debate_rounds")]
    pub debate_rounds: u32,
    /// Model for the judge (a SECOND, fresh Claude Code session). Empty = same model as the Claude debater.
    #[serde(default)]
    pub debate_judge_model: String,
    #[serde(default)]
    pub verify_gate: bool,
    #[serde(default)]
    pub verify_command: String,
    /// Verify command per project folder (key from `verify::folder_key`); wins over `verify_command`.
    #[serde(default)]
    pub verify_commands: std::collections::BTreeMap<String, String>,
    #[serde(default = "default_verify_ignore")]
    pub verify_ignore: Vec<String>,
    #[serde(default)]
    pub auto_review: bool,
    #[serde(default)]
    pub reviewer_provider_id: String,
    #[serde(default)]
    pub reviewer_model: String,
    #[serde(default)]
    pub auto_fix: bool,
    #[serde(default = "default_fix_rounds")]
    pub fix_rounds_max: u32,
    #[serde(default = "default_event_log")]
    pub event_log: bool,
    /// Sending a message at the MutiS source goes straight to "Chia việc" instead of a normal chat.
    #[serde(default)]
    pub auto_plan: bool,
    /// Off = a new plan is approved and run at once, and shown as ordinary chat messages.
    #[serde(default = "default_plan_approval")]
    pub plan_approval: bool,
    /// The user's own rules for the lead and the checker, added after the app's. The worker's are `worker_rules`.
    #[serde(default)]
    pub lead_rules: String,
    #[serde(default)]
    pub checker_rules: String,
    /// Off = the lead gets none of the app's general rules (CodeGraph, folder scope, preview) / the checker is not told to run the tests.
    /// The protocols the app reads back (delegation blocks, questions, the verdict JSON, Vietnamese prose) stay on regardless.
    #[serde(default = "default_enabled")]
    pub lead_app_rules: bool,
    #[serde(default = "default_enabled")]
    pub checker_app_rules: bool,
    /// MutiS High: an independent checker reviews each DeepSeek result before the coordinator judges it.
    #[serde(default)]
    pub delegate_checker: bool,
    /// MutiS High: whether DeepSeek may be given layout and styling work (.css/.html...). Off = such tasks go back to the coordinator.
    #[serde(default)]
    pub deepseek_ui: bool,
    /// Who the checker is (a Codex source and model; effort defaults to max). Used only at the High level.
    #[serde(default = "default_high_checker")]
    pub high_checker: MutisRole,
    #[serde(default = "default_routing")]
    pub routing: Vec<RoutingRule>,
    #[serde(default)]
    pub roles: MutisRoles,
    #[serde(default)]
    pub high_roles: MutisRoles,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRoles {
    pub lead: MutisRole,
    pub big: MutisRole,
    pub small: MutisRole,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRole {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub kind: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub assignee_kind: String,
    #[serde(default)]
    pub note: String,
}

impl Default for MutisConfig {
    fn default() -> Self {
        Self {
            worker_rules: default_worker_rules(),
            enabled: true,
            retry_api: false,
            retry_attempts: 3,
            retry_base_ms: 2000,
            codex_watchdog: false,
            watchdog_minutes: 5,
            codex_auto_resume: false,
            resume_max: 2,
            deepseek_agent: false,
            deepseek_max_file_lines: 400,
            deepseek_max_steps: 15,
            debate_rounds: 2,
            debate_judge_model: String::new(),
            verify_gate: false,
            verify_command: String::new(),
            verify_commands: Default::default(),
            verify_ignore: default_verify_ignore(),
            auto_review: false,
            reviewer_provider_id: String::new(),
            reviewer_model: String::new(),
            auto_fix: false,
            fix_rounds_max: 2,
            event_log: true,
            auto_plan: false,
            plan_approval: false,
            delegate_checker: false,
            lead_rules: String::new(),
            checker_rules: String::new(),
            lead_app_rules: true,
            checker_app_rules: true,
            deepseek_ui: false,
            high_checker: default_high_checker(),
            routing: default_routing(),
            roles: MutisRoles::default(),
            high_roles: MutisRoles::default(),
        }
    }
}
fn default_high_checker() -> MutisRole {
    MutisRole { provider_id: String::new(), model: String::new(), effort: "max".into() }
}
fn default_plan_approval() -> bool {
    false
}
fn default_worker_rules() -> String {
    crate::core::rule::worker_defaults()
}
impl MutisConfig {
    pub fn effective(&self, flag: bool) -> bool {
        self.enabled && flag
    }
    /// MutiS has one level, High: every effort uses `high_roles` (the old Max `roles` stay in the file only as High's fallback).
    pub fn roles_for(&self, _effort: &str) -> &MutisRoles {
        &self.high_roles
    }
    pub fn level_for(_effort: &str) -> &'static str {
        "high"
    }
}
fn default_deepseek_max_steps() -> u32 {
    15
}
fn default_debate_rounds() -> u32 {
    2
}
fn default_retry_attempts() -> u32 {
    3
}
fn default_retry_base_ms() -> u64 {
    2000
}
fn default_watchdog_minutes() -> u32 {
    5
}
fn default_resume_max() -> u32 {
    2
}
fn default_max_file_lines() -> u32 {
    400
}
fn default_fix_rounds() -> u32 {
    2
}
fn default_event_log() -> bool {
    true
}
/// MutiS is always on; this keeps `enabled` serialized as `true` and ignores incoming values.
fn default_enabled() -> bool {
    true
}
fn default_verify_ignore() -> Vec<String> {
    vec!["docs/**".into(), "**/*.md".into()]
}
fn default_routing() -> Vec<RoutingRule> {
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

fn default_effort() -> String {
    "default".into()
}

#[cfg(test)]
mod level_tests {
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
}
