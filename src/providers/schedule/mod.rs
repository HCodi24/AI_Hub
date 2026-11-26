//! One delegation round of MutiS. Independent tasks with disjoint files run together up to the configured worker slot count.
//! With an independent checker, every finished task is checked in parallel while the worker already
//! starts the next task that does not depend on it; severe findings come back as fix tasks at the end of the worker's queue.
//!
//! Only this loop writes to the chat (through `Backend::say`/`announce_check`) between worker runs, so the output of
//! the worker and of the checker never interleave.
use super::delegate::{Delegation, WorkerResult, MAX_ATTEMPTS};
use crate::core::text::truncate_chars;

mod check;
mod queue;
mod round;

pub use check::{check_text, parse_check, CheckResult};
pub(super) use round::run_round;

/// Worker slots use the same configured source, model and effort, with their own threads.
/// The independent tester only checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Actor {
    Worker(u8),
    Tester,
}

/// Attempts a task may use before the coordinator takes it over: with a checker and a coordinator that can edit there is
/// someone to take over, so three; otherwise the worker keeps all of them.
pub(super) fn limit(escalate: bool) -> usize {
    if escalate { 3 } else { MAX_ATTEMPTS }
}

/// One line of the live task board shown in the chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub title: String,
    /// waiting, coding, checking, fixwait (a fix is queued), fixing, done, failed, takeover (the editing coordinator now finishes it) or skipped.
    pub state: &'static str,
    pub attempt: usize,
    /// PASS, FAIL or UNCLEAR once the checker answered.
    pub check: Option<&'static str>,
    /// Actor and configured model for the current row state.
    pub actor: Option<String>,
    pub model: Option<String>,
    /// Actor and configured model retained for the latest check verdict.
    pub checker: Option<String>,
    pub checker_model: Option<String>,
    /// Short Vietnamese cause shown for takeover, failed-by-limit, or skipped rows.
    pub reason: Option<String>,
}

/// The coordinator's instruction when a task's attempts are spent; without write access it can only report what is left.
fn takeover_text(can_edit: bool) -> &'static str {
    if can_edit {
        "Implement it yourself now and verify the result."
    } else {
        "You cannot edit files in this chat (read-only): tell the user exactly what is left and why."
    }
}

#[async_trait::async_trait]
pub(super) trait Backend: Send + Sync + 'static {
    /// `editor` is the worker slot running the task.
    async fn work(&self, task: &Delegation, attempt: usize, feedback: Option<&str>, editor: Actor) -> WorkerResult;
    fn has_checker(&self) -> bool;
    /// Maximum number of independent tasks that can run at once.
    fn worker_slots(&self) -> usize { 1 }
    /// Runs in its own task, in parallel with the worker; must not write to the chat.
    async fn check(&self, task: &Delegation, attempt: usize, work: &WorkerResult) -> CheckResult;
    async fn say(&self, line: String);
    async fn announce_check(&self, task: &Delegation, attempt: usize, check: &CheckResult);
    fn cancelled(&self) -> bool;
    /// Why the source behind this actor cannot be called now (its usage limit is spent), if so.
    fn limited(&self, _actor: Actor) -> Option<String> { None }
    /// The actor's source just answered with a usage-limit error: stop calling it for a while.
    fn note_limit(&self, _actor: Actor, _why: &str) {}
    /// False when the coordinator may not edit files, so it cannot take a task over.
    fn can_edit(&self) -> bool;
    /// Name shown for the worker slot or checker.
    fn actor_name(&self, actor: Actor) -> String;
    /// Configured model for the actor selected by the same route used for execution.
    fn actor_model(&self, _actor: Actor) -> Option<String> { None }
    /// Shows the whole task board; called only from the round loop.
    async fn board(&self, rows: &[Row]);
    fn path_problems(&self, task: &Delegation) -> Vec<String>;
    /// Escalation needs someone to check and a coordinator that can finish the task.
    fn can_escalate(&self) -> bool {
        self.has_checker() && self.can_edit()
    }
}

#[derive(Default)]
pub(super) struct RoundOutcome {
    /// One entry per worker run, check, skipped task or spent limit, in order, for the coordinator.
    pub reports: Vec<String>,
    /// Low-severity findings: not sent to the worker, only to the coordinator.
    pub minor: Vec<String>,
    pub changed: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Checking,
    Clean,
    Failed,
}

/// Told to the coordinator when a source is out of its usage limit: no retry happened, and the choice is its own.
fn limit_text(title: &str, who: &str, why: &str) -> String {
    format!("Task \"{}\" was NOT done: {who} has used up its usage limit and is not called again for a while ({}). The app did not retry and did not hand it over. Tell the user, then wait for the limit to reset, ask for another source, or implement the task yourself if its own limit allows.", title.trim(), truncate_chars(why, 200))
}

#[cfg(test)]
mod tests;
