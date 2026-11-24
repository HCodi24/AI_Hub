use serde::{Deserialize, Serialize};

mod defaults;
mod roles;

use defaults::*;
pub use roles::{MutisChecker, MutisRole, MutisWorker};

#[cfg(test)]
mod tests;

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
    /// An independent checker reviews delegated work before the lead judges it.
    #[serde(default)]
    pub delegate_checker: bool,
    #[serde(default = "default_lead")]
    pub lead: MutisRole,
    #[serde(default)]
    pub worker: MutisWorker,
    #[serde(default)]
    pub checker: MutisChecker,
}

impl Default for MutisConfig {
    fn default() -> Self {
        Self {
            worker_rules: default_worker_rules(),
            enabled: default_enabled(),
            retry_api: false,
            retry_attempts: default_retry_attempts(),
            retry_base_ms: default_retry_base_ms(),
            codex_watchdog: false,
            watchdog_minutes: default_watchdog_minutes(),
            codex_auto_resume: false,
            resume_max: default_resume_max(),
            deepseek_agent: false,
            deepseek_max_file_lines: default_max_file_lines(),
            deepseek_max_steps: default_deepseek_max_steps(),
            debate_rounds: default_debate_rounds(),
            debate_judge_model: String::new(),
            verify_gate: false,
            verify_command: String::new(),
            verify_commands: Default::default(),
            verify_ignore: default_verify_ignore(),
            auto_review: false,
            reviewer_provider_id: String::new(),
            reviewer_model: String::new(),
            auto_fix: false,
            fix_rounds_max: default_fix_rounds(),
            event_log: default_event_log(),
            auto_plan: false,
            plan_approval: default_plan_approval(),
            delegate_checker: false,
            lead_rules: String::new(),
            checker_rules: String::new(),
            lead_app_rules: default_enabled(),
            checker_app_rules: default_enabled(),
            lead: default_lead(),
            worker: MutisWorker::default(),
            checker: MutisChecker::default(),
        }
    }
}

impl MutisConfig {
    pub fn effective(&self, flag: bool) -> bool {
        self.enabled && flag
    }
}
