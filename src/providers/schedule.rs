//! One delegation round of MutiS High. The worker runs the tasks one after another without pause. With an independent
//! checker, every finished task is checked in parallel while the worker already starts the next task that does not
//! depend on it; severe findings come back as fix tasks at the end of the worker's queue.
//!
//! Only this loop writes to the chat (through `Backend::say`/`announce_check`) between worker runs, so the output of
//! the worker and of the checker never interleave.
use super::delegate::{auto_retry_reason, report, Delegation, WorkerResult, MAX_ATTEMPTS, MAX_AUTO_RETRIES};
use crate::core::text::{extract_json, truncate_chars};
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Unclear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: String,
    pub file: String,
    pub problem: String,
    pub fix: String,
}

impl Issue {
    /// High and medium findings make the worker redo the task; low ones only go to the coordinator.
    pub fn is_severe(&self) -> bool {
        matches!(self.severity.to_lowercase().as_str(), "high" | "medium" | "critical" | "blocker")
    }
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub verdict: Verdict,
    pub issues: Vec<Issue>,
    pub notes: String,
    pub error: Option<String>,
}

impl CheckResult {
    pub(super) fn unclear(why: &str) -> Self {
        Self { verdict: Verdict::Unclear, issues: Vec::new(), notes: String::new(), error: Some(why.to_owned()) }
    }
}

/// The checker's JSON answer; anything unreadable is "unclear" (reported, never turned into a fix task).
pub fn parse_check(text: &str) -> CheckResult {
    let Some(json) = extract_json(text) else { return CheckResult::unclear("the checker did not answer with JSON") };
    let Ok(value) = serde_json::from_str::<Value>(json) else { return CheckResult::unclear("the checker's JSON could not be read") };
    let verdict = match value["verdict"].as_str().map(|v| v.trim().to_uppercase()).as_deref() {
        Some("PASS") => Verdict::Pass,
        Some("FAIL") => Verdict::Fail,
        _ => Verdict::Unclear,
    };
    let text_of = |item: &Value, key: &str| item[key].as_str().unwrap_or_default().trim().to_owned();
    let issues = value["issues"].as_array().into_iter().flatten().map(|item| Issue {
        severity: if text_of(item, "severity").is_empty() { "medium".into() } else { text_of(item, "severity") },
        file: text_of(item, "file"),
        problem: text_of(item, "problem"),
        fix: text_of(item, "fix"),
    }).filter(|issue| !issue.problem.is_empty()).collect();
    CheckResult { verdict, issues, notes: text_of(&value, "notes"), error: None }
}

/// What the chat shows for one check, and what the coordinator reads.
pub fn check_text(task: &Delegation, attempt: usize, check: &CheckResult) -> String {
    let verdict = match check.verdict {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Unclear => "UNCLEAR",
    };
    let mut text = format!("Independent check for \"{}\" (attempt {attempt}): {verdict}\n", task.title.trim());
    if let Some(error) = &check.error {
        text.push_str(&format!("Checker problem: {error}\n"));
    }
    for issue in &check.issues {
        text.push_str(&format!("- [{}] {}: {} -> {}\n", issue.severity, issue.file, issue.problem, issue.fix));
    }
    if !check.notes.is_empty() {
        text.push_str(&format!("Notes: {}\n", check.notes));
    }
    text
}

const MAX_TASK_FILES: usize = 12;

/// File paths named in a checker issue's `file` field: several may be joined by `;` or `,`, and each may carry a
/// `:line` suffix or backticks. Anything with a space is not a path.
pub fn issue_files(raw: &str) -> Vec<String> {
    raw.split([';', ',']).map(|part| {
        let part = part.trim().trim_matches('`').trim_start_matches("./");
        let path = match part.rsplit_once(':') {
            Some((path, line)) if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit() || c == '-') => path,
            _ => part,
        };
        path.trim().to_owned()
    }).filter(|path| !path.is_empty() && !path.contains(' ')).collect()
}

fn same_path(a: &str, b: &str) -> bool {
    let clean = |p: &str| p.replace('\\', "/").trim_start_matches("./").to_lowercase();
    clean(a) == clean(b)
}

/// The same task again, with the severe findings added to its instructions. The title stays: the 4 attempts are shared.
fn fix_task(task: &Delegation, severe: &[&Issue]) -> Delegation {
    let list = severe.iter().map(|i| format!("- {}: {} (fix: {})", i.file, i.problem, i.fix)).collect::<Vec<_>>().join("\n");
    Delegation {
        instructions: format!("{}\n\nAn independent check of your previous result found these problems. Fix exactly these and keep everything else:\n{list}", task.instructions.trim()),
        ..task.clone()
    }
}

/// Who edits and who checks. Luna is the independent checker's Codex thread; Luna 2 is a second Codex thread (max effort).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Actor {
    DeepSeek,
    Luna,
    Luna2,
}

impl Actor {
    pub(super) fn name(self) -> &'static str {
        match self {
            Actor::DeepSeek => "DeepSeek",
            Actor::Luna => "Luna",
            Actor::Luna2 => "Luna 2",
        }
    }
}

/// (editor, checker) of one attempt. With a checker and a coordinator that can edit the task climbs a ladder:
/// DeepSeek twice (Luna checks), then Luna fixes it and Luna 2 checks; a `hard` task starts with Luna 2 (Luna checks).
/// Without that, DeepSeek does every attempt.
pub(super) fn route(hard: bool, attempt: usize, escalate: bool) -> (Actor, Actor) {
    match (escalate, hard, attempt) {
        (true, true, _) => (Actor::Luna2, Actor::Luna),
        (true, false, 3..) => (Actor::Luna, Actor::Luna2),
        _ => (Actor::DeepSeek, Actor::Luna),
    }
}

/// Attempts a task may use before the coordinator takes it over.
pub(super) fn limit(hard: bool, escalate: bool) -> usize {
    match (escalate, hard) {
        (true, true) => 2,
        (true, false) => 3,
        _ => MAX_ATTEMPTS,
    }
}

/// One line of the live task board shown in the chat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Row {
    pub title: String,
    /// waiting, coding, checking, fixing, done, failed, takeover (the editing coordinator now finishes it) or skipped.
    pub state: &'static str,
    pub attempt: usize,
    /// PASS, FAIL or UNCLEAR once the checker answered.
    pub check: Option<&'static str>,
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
    async fn work(&self, task: &Delegation, attempt: usize, feedback: Option<&str>) -> WorkerResult;
    fn has_checker(&self) -> bool;
    /// Runs in its own task, in parallel with the worker; must not write to the chat.
    async fn check(&self, task: &Delegation, attempt: usize, work: &WorkerResult) -> CheckResult;
    async fn say(&self, line: String);
    async fn announce_check(&self, task: &Delegation, attempt: usize, check: &CheckResult);
    fn cancelled(&self) -> bool;
    /// False when the coordinator may not edit files, so it cannot take a task over.
    fn can_edit(&self) -> bool;
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

fn key(title: &str) -> String {
    title.trim().to_lowercase()
}

enum Next {
    Run(Delegation),
    Skip(Delegation, String),
    Wait,
}

/// The first queued task whose dependencies are settled. Independent tasks never wait for a check.
fn next_ready(queue: &mut VecDeque<Delegation>, state: &HashMap<String, State>) -> Next {
    let queued: Vec<String> = queue.iter().map(|t| key(&t.title)).collect();
    for index in 0..queue.len() {
        let own = key(&queue[index].title);
        let (mut blocked, mut waiting) = (None, false);
        for dep in queue[index].depends_on.iter().map(|d| key(d)).filter(|d| *d != own) {
            match state.get(&dep) {
                Some(State::Failed) => blocked = Some(dep),
                Some(State::Checking) => waiting = true,
                Some(State::Clean) => {}
                None => waiting |= queued.contains(&dep),
            }
        }
        if let Some(dep) = blocked {
            return Next::Skip(queue.remove(index).expect("index is in range"), format!("it depends on \"{dep}\", which did not pass"));
        }
        if !waiting {
            return Next::Run(queue.remove(index).expect("index is in range"));
        }
    }
    Next::Wait
}

type Finished = (Delegation, usize, CheckResult);
type Job = (Delegation, usize, WorkerResult);

struct Round<'a, B: Backend> {
    backend: &'a Arc<B>,
    queue: VecDeque<Delegation>,
    state: HashMap<String, State>,
    attempts: &'a mut HashMap<String, usize>,
    jobs: Option<mpsc::UnboundedSender<Job>>,
    pending: usize,
    out: RoundOutcome,
    rows: Vec<Row>,
}

impl<B: Backend> Round<'_, B> {
    /// Updates one row of the board (rows are keyed by the task title) and shows the board.
    async fn mark(&mut self, title: &str, state: &'static str, attempt: usize, check: Option<&'static str>) {
        let wanted = key(title);
        if let Some(row) = self.rows.iter_mut().find(|row| key(&row.title) == wanted) {
            row.state = state;
            row.attempt = attempt.max(row.attempt);
            if check.is_some() {
                row.check = check;
            }
        }
        self.backend.board(&self.rows).await;
    }

    /// A spent task is handed to the coordinator when it can edit; a read-only one can only report it.
    fn spent_state(&self) -> &'static str {
        if self.backend.can_edit() { "takeover" } else { "failed" }
    }

    async fn run_task(&mut self, task: Delegation) {
        let title = key(&task.title);
        let escalate = self.backend.can_escalate();
        let max = limit(task.hard, escalate);
        if self.attempts.get(&title).copied().unwrap_or(0) >= max {
            self.out.reports.push(format!("The attempt limit ({max}/{max}) for \"{}\" is spent. {}", task.title.trim(), takeover_text(self.backend.can_edit())));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, self.spent_state(), max, None).await;
            return;
        }
        let problems = self.backend.path_problems(&task);
        if !problems.is_empty() {
            self.out.reports.push(format!("Path check failed for \"{}\": the worker was NOT run and this is not counted as an attempt. {}. Resend the block with correct paths.", task.title.trim(), problems.join("; ")));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, "failed", 0, None).await;
            return;
        }
        let (mut auto, mut feedback) = (0, None::<String>);
        let (attempt, work) = loop {
            let used = self.attempts.entry(title.clone()).or_insert(0);
            *used += 1;
            let attempt = *used;
            let fixing = attempt > 1 && auto == 0;
            self.mark(&task.title, if fixing { "fixing" } else { "coding" }, attempt, None).await;
            let work = self.backend.work(&task, attempt, feedback.as_deref()).await;
            if self.backend.cancelled() {
                return;
            }
            match auto_retry_reason(&work) {
                Some(why) if auto < MAX_AUTO_RETRIES && attempt < max => {
                    auto += 1;
                    self.backend.say(format!("Tự giao lại {} ({auto}/{MAX_AUTO_RETRIES}): {}", route(task.hard, attempt, escalate).0.name(), truncate_chars(&why, 160))).await;
                    feedback = Some(why);
                }
                _ => break (attempt, work),
            }
        };
        let (editor, checker) = route(task.hard, attempt, escalate);
        self.out.reports.push(report(&task, editor.name(), attempt, max, auto, &work));
        if auto_retry_reason(&work).is_some() {
            self.state.insert(title, State::Failed);
            self.mark(&task.title, self.spent_state(), attempt, None).await;
            return;
        }
        self.out.changed = true;
        let Some(jobs) = self.jobs.clone() else {
            self.state.insert(title, State::Clean);
            self.mark(&task.title, "done", attempt, None).await;
            return;
        };
        self.backend.say(format!("{} kiểm tra: {} (lần {attempt})", checker.name(), task.title.trim())).await;
        self.state.insert(title.clone(), State::Checking);
        self.mark(&task.title, "checking", attempt, None).await;
        self.pending += 1;
        let shown = task.title.clone();
        if jobs.send((task, attempt, work)).is_err() {
            self.pending -= 1;
            self.state.insert(title, State::Clean);
            self.mark(&shown, "done", attempt, None).await;
        }
    }

    async fn handle_check(&mut self, (task, attempt, check): Finished) {
        self.pending -= 1;
        self.backend.announce_check(&task, attempt, &check).await;
        self.out.reports.push(check_text(&task, attempt, &check));
        let title = key(&task.title);
        let shown = match check.verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Unclear => "UNCLEAR",
        };
        let severe: Vec<&Issue> = check.issues.iter().filter(|i| i.is_severe()).collect();
        self.out.minor.extend(check.issues.iter().filter(|i| !i.is_severe()).map(|i| format!("{}: {} ({})", task.title.trim(), i.problem, i.file)));
        if check.verdict == Verdict::Fail && !severe.is_empty() {
            let max = limit(task.hard, self.backend.can_escalate());
            if self.attempts.get(&title).copied().unwrap_or(0) < max {
                self.state.remove(&title);
                let mut fix = fix_task(&task, &severe);
                // The finding may sit in a file the coordinator did not list; the worker could not touch it and the attempts would be wasted.
                let mut added = Vec::new();
                for file in severe.iter().flat_map(|issue| issue_files(&issue.file)) {
                    if fix.files.len() >= MAX_TASK_FILES || fix.files.iter().any(|known| same_path(known, &file)) {
                        continue;
                    }
                    let probe = Delegation { files: vec![file.clone()], ..fix.clone() };
                    if self.backend.path_problems(&probe).is_empty() {
                        fix.files.push(file.clone());
                        added.push(file);
                    }
                }
                if !added.is_empty() {
                    self.backend.say(format!("Thêm file Luna nêu vào task sửa: {}", added.join(", "))).await;
                }
                self.queue.push_back(fix);
                self.mark(&task.title, "fixing", attempt, Some(shown)).await;
            } else {
                self.out.reports.push(format!("The attempt limit ({max}/{max}) for \"{}\" is spent and the check still fails. {}", task.title.trim(), takeover_text(self.backend.can_edit())));
                self.state.insert(title, State::Failed);
                self.mark(&task.title, self.spent_state(), attempt, Some(shown)).await;
            }
            return;
        }
        self.state.insert(title, State::Clean);
        self.mark(&task.title, "done", attempt, Some(shown)).await;
    }
}

/// Runs the tasks of one coordinator reply to the end. `attempts` (per task title) lives across rounds.
pub(super) async fn run_round<B: Backend>(backend: &Arc<B>, tasks: Vec<Delegation>, attempts: &mut HashMap<String, usize>) -> RoundOutcome {
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Finished>();
    // One checker at a time, so its conversation (a resumed thread) stays in order.
    let jobs = backend.has_checker().then(|| {
        let (job_tx, mut job_rx) = mpsc::unbounded_channel::<Job>();
        let backend = backend.clone();
        tokio::spawn(async move {
            while let Some((task, attempt, work)) = job_rx.recv().await {
                let result = backend.check(&task, attempt, &work).await;
                if done_tx.send((task, attempt, result)).is_err() {
                    break;
                }
            }
        });
        job_tx
    });
    let mut rows: Vec<Row> = Vec::new();
    for task in &tasks {
        if !rows.iter().any(|row| key(&row.title) == key(&task.title)) {
            rows.push(Row { title: task.title.trim().to_owned(), state: "waiting", attempt: 0, check: None });
        }
    }
    let mut round = Round { backend, queue: tasks.into(), state: HashMap::new(), attempts, jobs, pending: 0, out: RoundOutcome::default(), rows };
    round.backend.board(&round.rows).await;
    while !backend.cancelled() {
        while let Ok(finished) = done_rx.try_recv() {
            round.handle_check(finished).await;
        }
        match next_ready(&mut round.queue, &round.state) {
            Next::Run(task) => round.run_task(task).await,
            Next::Skip(task, why) => {
                round.out.reports.push(format!("Task \"{}\" was skipped: {why}.", task.title.trim()));
                round.state.insert(key(&task.title), State::Failed);
                round.mark(&task.title, "skipped", 0, None).await;
            }
            Next::Wait if round.pending > 0 => match done_rx.recv().await {
                Some(finished) => round.handle_check(finished).await,
                None => break,
            },
            Next::Wait => break,
        }
    }
    for task in std::mem::take(&mut round.queue) {
        round.out.reports.push(format!("Task \"{}\" was not run: its dependencies never settled.", task.title.trim()));
    }
    round.out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::time::Duration;
    use tokio::sync::Notify;

    fn task(title: &str, deps: &[&str]) -> Delegation {
        Delegation { title: title.into(), files: vec!["a.rs".into()], instructions: "do it".into(), acceptance: vec![], depends_on: deps.iter().map(|d| d.to_string()).collect(), hard: false }
    }
    fn fail_high() -> CheckResult {
        CheckResult { verdict: Verdict::Fail, issues: vec![Issue { severity: "high".into(), file: "a.rs".into(), problem: "wrong".into(), fix: "right".into() }], notes: String::new(), error: None }
    }
    fn pass() -> CheckResult {
        CheckResult { verdict: Verdict::Pass, issues: vec![], notes: String::new(), error: None }
    }

    /// Records the order of events; `checks` scripts the checker's answers per title (default PASS).
    #[derive(Default)]
    struct Fake {
        log: Mutex<Vec<String>>,
        checker: bool,
        checks: Mutex<HashMap<String, VecDeque<CheckResult>>>,
        empty: Vec<String>,
        /// The check of this title waits until the worker has started the other title: proves the two run in parallel.
        hold: Option<(String, String)>,
        released: Notify,
        read_only: bool,
        boards: Mutex<Vec<String>>,
        /// Paths the fake project does not accept.
        bad_paths: Vec<String>,
        /// Title and files of every worker run.
        works: Mutex<Vec<(String, Vec<String>)>>,
    }
    impl Fake {
        fn log(&self, line: impl Into<String>) {
            self.log.lock().unwrap().push(line.into());
        }
        fn lines(&self) -> Vec<String> {
            self.log.lock().unwrap().clone()
        }
    }
    #[async_trait::async_trait]
    impl Backend for Fake {
        async fn work(&self, task: &Delegation, _attempt: usize, _feedback: Option<&str>) -> WorkerResult {
            self.log(format!("work:{}", task.title));
            self.works.lock().unwrap().push((task.title.clone(), task.files.clone()));
            if self.hold.as_ref().is_some_and(|(_, other)| *other == task.title) {
                self.released.notify_one();
            }
            let diff = if self.empty.contains(&task.title) { "" } else { "+x" };
            WorkerResult { summary: String::new(), diff: diff.into(), error: None, refused: Vec::new() }
        }
        fn has_checker(&self) -> bool {
            self.checker
        }
        async fn check(&self, task: &Delegation, _attempt: usize, _work: &WorkerResult) -> CheckResult {
            if self.hold.as_ref().is_some_and(|(held, _)| *held == task.title) {
                self.released.notified().await;
            }
            self.log(format!("check:{}", task.title));
            self.checks.lock().unwrap().get_mut(&task.title).and_then(|q| q.pop_front()).unwrap_or_else(pass)
        }
        async fn say(&self, _line: String) {}
        async fn announce_check(&self, task: &Delegation, _attempt: usize, _check: &CheckResult) {
            self.log(format!("announce:{}", task.title));
        }
        fn cancelled(&self) -> bool {
            false
        }
        fn can_edit(&self) -> bool {
            !self.read_only
        }
        async fn board(&self, rows: &[Row]) {
            self.boards.lock().unwrap().push(rows.iter().map(|r| format!("{}:{}", r.title, r.state)).collect::<Vec<_>>().join(","));
        }
        fn path_problems(&self, task: &Delegation) -> Vec<String> {
            if task.files.iter().any(|file| self.bad_paths.contains(file)) { vec!["not a project path".into()] } else { Vec::new() }
        }
    }
    async fn run(fake: Fake, tasks: Vec<Delegation>) -> (Arc<Fake>, RoundOutcome, HashMap<String, usize>) {
        let backend = Arc::new(fake);
        let mut attempts = HashMap::new();
        let out = tokio::time::timeout(Duration::from_secs(5), run_round(&backend, tasks, &mut attempts)).await.expect("the round must not hang");
        (backend, out, attempts)
    }

    #[tokio::test]
    async fn without_a_checker_tasks_run_in_order() {
        let (fake, out, _) = run(Fake::default(), vec![task("A", &[]), task("B", &["A"])]).await;
        assert_eq!(fake.lines(), ["work:A", "work:B"]);
        assert_eq!(out.reports.len(), 2);
        assert!(out.changed);
    }

    #[tokio::test]
    async fn the_worker_starts_the_next_task_while_the_previous_one_is_checked() {
        let fake = Fake { checker: true, hold: Some(("A".into(), "B".into())), ..Default::default() };
        let (fake, out, _) = run(fake, vec![task("A", &[]), task("B", &[])]).await;
        let lines = fake.lines();
        let at = |what: &str| lines.iter().position(|l| l == what).unwrap();
        assert!(at("work:B") < at("check:A"), "B started before A's check finished: {lines:?}");
        assert!(lines.contains(&"announce:A".to_string()) && lines.contains(&"announce:B".to_string()));
        assert_eq!(out.reports.iter().filter(|r| r.starts_with("Independent check")).count(), 2);
    }

    #[tokio::test]
    async fn a_dependent_task_waits_for_the_check_of_its_dependency() {
        let fake = Fake { checker: true, ..Default::default() };
        let (fake, _, _) = run(fake, vec![task("A", &[]), task("B", &["A"])]).await;
        let lines = fake.lines();
        let at = |what: &str| lines.iter().position(|l| l == what).unwrap();
        assert!(at("check:A") < at("work:B"), "{lines:?}");
    }

    #[tokio::test]
    async fn a_severe_finding_queues_a_fix_for_the_same_task_and_shares_its_attempts() {
        let fake = Fake { checker: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from([fail_high(), pass()]));
        let (fake, out, attempts) = run(fake, vec![task("A", &[]), task("B", &[])]).await;
        let works = fake.lines().iter().filter(|l| l.starts_with("work:A")).count();
        assert_eq!(works, 2, "the fix is a second worker run of A: {:?}", fake.lines());
        assert_eq!(attempts["a"], 2);
        assert!(out.reports.iter().any(|r| r.contains("FAIL")) && out.reports.iter().any(|r| r.contains("PASS")));
    }

    #[tokio::test]
    async fn a_check_that_never_passes_stops_at_the_attempt_limit() {
        let fake = Fake { checker: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from(vec![fail_high(); 10]));
        let (fake, out, attempts) = run(fake, vec![task("A", &[])]).await;
        // DeepSeek twice, then Luna's fix: the third failed check hands the task to the coordinator.
        assert_eq!(fake.lines().iter().filter(|l| l.starts_with("work:")).count(), 3);
        assert_eq!(attempts["a"], 3);
        assert!(out.reports.last().unwrap().contains("attempt limit (3/3)"));
        // The fake can edit, so the spent task is handed to the coordinator instead of showing a bare failure.
        assert_eq!(fake.boards.lock().unwrap().last().unwrap(), "A:takeover");
    }

    #[tokio::test]
    async fn a_hard_task_gets_two_attempts_and_without_a_checker_every_task_keeps_four() {
        let fake = Fake { checker: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from(vec![fail_high(); 10]));
        let hard = Delegation { hard: true, ..task("A", &[]) };
        let (fake, out, attempts) = run(fake, vec![hard]).await;
        assert_eq!((fake.lines().iter().filter(|l| l.starts_with("work:")).count(), attempts["a"]), (2, 2));
        assert!(out.reports.last().unwrap().contains("attempt limit (2/2)"));
        // No checker, or a coordinator that cannot edit: nobody to climb to, so the old four DeepSeek attempts stay.
        let (fake, _, attempts) = run(Fake { empty: vec!["A".into()], ..Default::default() }, vec![Delegation { hard: true, ..task("A", &[]) }]).await;
        assert!(attempts["a"] <= MAX_ATTEMPTS && fake.lines().iter().all(|l| l.starts_with("work:")));
        assert_eq!(limit(true, false), MAX_ATTEMPTS);
        assert_eq!(limit(false, false), MAX_ATTEMPTS);
    }

    #[test]
    fn the_ladder_is_deepseek_twice_then_luna_fixes_and_luna_2_checks_and_hard_tasks_start_with_luna_2() {
        use Actor::*;
        assert_eq!([1, 2, 3].map(|n| route(false, n, true)), [(DeepSeek, Luna), (DeepSeek, Luna), (Luna, Luna2)]);
        assert_eq!([1, 2].map(|n| route(true, n, true)), [(Luna2, Luna), (Luna2, Luna)]);
        assert_eq!([1, 3, 4].map(|n| route(true, n, false)), [(DeepSeek, Luna); 3]);
        assert_eq!(route(false, 4, false), (DeepSeek, Luna));
    }

    #[tokio::test]
    async fn low_findings_do_not_trigger_a_fix_and_a_dependent_of_a_failed_task_is_skipped() {
        let low = CheckResult { verdict: Verdict::Fail, issues: vec![Issue { severity: "low".into(), file: "a.rs".into(), problem: "naming".into(), fix: "rename".into() }], notes: String::new(), error: None };
        let fake = Fake { checker: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from([low]));
        let (fake, out, _) = run(fake, vec![task("A", &[])]).await;
        assert_eq!(fake.lines().iter().filter(|l| l.starts_with("work:")).count(), 1);
        assert_eq!(out.minor.len(), 1);
        let fake = Fake { empty: vec!["A".into()], ..Default::default() };
        let (fake, out, _) = run(fake, vec![task("A", &[]), task("B", &["A"])]).await;
        assert!(!fake.lines().contains(&"work:B".to_string()));
        assert!(out.reports.iter().any(|r| r.contains("skipped")));
    }

    #[tokio::test]
    async fn the_board_follows_each_task_from_waiting_to_done_and_shows_fixes() {
        let fake = Fake { checker: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from([fail_high(), pass()]));
        let (fake, _, _) = run(fake, vec![task("A", &[]), task("B", &["A"])]).await;
        let boards = fake.boards.lock().unwrap().clone();
        assert_eq!(boards[0], "A:waiting,B:waiting");
        for wanted in ["A:coding,B:waiting", "A:checking,B:waiting", "A:fixing,B:waiting", "A:done,B:done"] {
            assert!(boards.iter().any(|b| b == wanted), "missing board {wanted}: {boards:?}");
        }
    }

    #[tokio::test]
    async fn a_read_only_coordinator_is_not_told_to_implement_the_task_itself() {
        let fake = Fake { checker: true, read_only: true, ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from(vec![fail_high(); 10]));
        let (_, out, _) = run(fake, vec![task("A", &[])]).await;
        let last = out.reports.last().unwrap();
        assert!(last.contains("cannot edit files") && !last.contains("Implement it yourself"), "{last}");
    }

    #[tokio::test]
    async fn a_fix_task_also_gets_the_valid_files_the_checker_named() {
        let issue = Issue { severity: "high".into(), file: "b.rs; c.rs:12; `d.rs`; ../x.rs; ./A.rs".into(), problem: "p".into(), fix: "f".into() };
        let failing = CheckResult { verdict: Verdict::Fail, issues: vec![issue], notes: String::new(), error: None };
        let fake = Fake { checker: true, bad_paths: vec!["../x.rs".into()], ..Default::default() };
        fake.checks.lock().unwrap().insert("A".into(), VecDeque::from([failing, pass()]));
        let (fake, _, _) = run(fake, vec![task("A", &[])]).await;
        let works = fake.works.lock().unwrap().clone();
        assert_eq!(works[0].1, ["a.rs"], "the first run keeps the coordinator's list");
        assert_eq!(works[1].1, ["a.rs", "b.rs", "c.rs", "d.rs"], "valid files added once, the invalid path and the duplicate (other case, ./) dropped");
    }

    #[test]
    fn issue_files_splits_joined_paths_and_drops_line_numbers_and_prose() {
        assert_eq!(issue_files("src/a.rs; src/b.rs:12-20, `web/c.ts`"), ["src/a.rs", "src/b.rs", "web/c.ts"]);
        assert!(issue_files("").is_empty() && issue_files("the whole crate").is_empty());
        assert_eq!(issue_files("./x.rs:7"), ["x.rs"]);
    }

    #[tokio::test]
    async fn a_dependency_cycle_ends_instead_of_hanging() {
        let (_, out, _) = run(Fake::default(), vec![task("A", &["B"]), task("B", &["A"])]).await;
        assert_eq!(out.reports.iter().filter(|r| r.contains("never settled")).count(), 2);
    }

    #[test]
    fn the_checker_answer_is_parsed_and_unreadable_answers_are_unclear() {
        let ok = parse_check("Here: {\"verdict\":\"fail\",\"issues\":[{\"severity\":\"HIGH\",\"file\":\"a.rs\",\"problem\":\"p\",\"fix\":\"f\"},{\"problem\":\"\"}],\"notes\":\"n\"}");
        assert_eq!(ok.verdict, Verdict::Fail);
        assert_eq!(ok.issues.len(), 1, "an issue without a problem is dropped");
        assert!(ok.issues[0].is_severe() && ok.notes == "n");
        assert_eq!(parse_check("looks fine to me").verdict, Verdict::Unclear);
        assert_eq!(parse_check("{\"verdict\":\"maybe\"}").verdict, Verdict::Unclear);
        assert!(!Issue { severity: "low".into(), file: String::new(), problem: "x".into(), fix: String::new() }.is_severe());
    }
}
