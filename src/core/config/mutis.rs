use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutisConfig {
    #[serde(default = "default_worker_rules")]
    pub worker_rules: String,
    #[serde(default)]
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
            enabled: false,
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
            verify_ignore: default_verify_ignore(),
            auto_review: false,
            reviewer_provider_id: String::new(),
            reviewer_model: String::new(),
            auto_fix: false,
            fix_rounds_max: 2,
            event_log: true,
            auto_plan: false,
            routing: default_routing(),
            roles: MutisRoles::default(),
            high_roles: MutisRoles::default(),
        }
    }
}
fn default_worker_rules() -> String {
    crate::core::rule::worker_defaults()
}
impl MutisConfig {
    pub fn effective(&self, flag: bool) -> bool {
        self.enabled && flag
    }
    pub fn roles_for(&self, effort: &str) -> &MutisRoles {
        if matches!(effort, "max" | "xhigh") { &self.roles } else { &self.high_roles }
    }
    pub fn level_for(effort: &str) -> &'static str {
        if matches!(effort, "max" | "xhigh") { "max" } else { "high" }
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
    fn effort_selects_max_or_high_roles() {
        let mut config = MutisConfig::default();
        config.roles.lead.provider_id = "max".into();
        config.high_roles.lead.provider_id = "high".into();
        for effort in ["max", "xhigh"] { assert_eq!(config.roles_for(effort).lead.provider_id, "max"); }
        for effort in ["default", "low", "medium", "high"] { assert_eq!(config.roles_for(effort).lead.provider_id, "high"); }
        assert_eq!(MutisConfig::level_for("xhigh"), "max");
        assert_eq!(MutisConfig::level_for("medium"), "high");
    }
}
