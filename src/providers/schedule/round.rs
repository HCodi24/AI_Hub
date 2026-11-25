use super::check::{check_text, fix_task, issue_files, same_path, CheckResult, Issue, Verdict, MAX_TASK_FILES};
use super::queue::{key, next_ready, take_partner, Next};
use super::{limit, limit_text, takeover_text, Actor, Backend, RoundOutcome, Row, State};
use crate::core::text::truncate_chars;
use crate::features::providers::limits::quota;
use crate::providers::delegate::{auto_retry_reason, report, Delegation, WorkerResult, MAX_AUTO_RETRIES};
use futures_util::future::join_all;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::mpsc;

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
    async fn mark(&mut self, title: &str, state: &'static str, attempt: usize, check: Option<&'static str>, actor: Option<Actor>, model: Option<String>, checker: Option<Actor>, checker_model: Option<String>) {
        let wanted = key(title);
        if let Some(row) = self.rows.iter_mut().find(|row| key(&row.title) == wanted) {
            row.state = state;
            row.attempt = attempt.max(row.attempt);
            if check.is_some() {
                row.check = check;
            }
            if let Some(actor) = actor {
                row.actor = Some(self.backend.actor_name(actor));
                row.model = model;
            }
            if let Some(checker) = checker {
                row.checker = Some(self.backend.actor_name(checker));
                row.checker_model = checker_model;
            }
        }
        self.backend.board(&self.rows).await;
    }

    /// A spent task is handed to the coordinator when it can edit; a read-only one can only report it.
    fn spent_state(&self) -> &'static str {
        if self.backend.can_edit() { "takeover" } else { "failed" }
    }

    /// Runs the first attempt of each task on a distinct worker slot.
    async fn run_group(&mut self, group: Vec<Delegation>) {
        let mut started = Vec::new();
        for (slot, task) in (0u8..).zip(group) {
            let editor = Actor::Worker(slot);
            if let Some(attempt) = self.begin(&task, editor).await {
                started.push((task, editor, attempt));
            }
        }
        // Only the first call of each task is concurrent; the app's retries and the checks stay in this loop's order.
        let backend = self.backend;
        let works = join_all(started.iter().map(|(task, editor, attempt)| {
            backend.work(task, *attempt, None, *editor)
        })).await;
        for ((task, editor, attempt), work) in started.into_iter().zip(works) {
            self.settle(task, editor, attempt, work).await;
        }
    }

    /// Checks the limit and paths before counting the first attempt; None = nothing runs.
    async fn begin(&mut self, task: &Delegation, editor: Actor) -> Option<usize> {
        let title = key(&task.title);
        let max = limit(self.backend.can_escalate());
        if self.attempts.get(&title).copied().unwrap_or(0) >= max {
            self.out.reports.push(format!("The attempt limit ({max}/{max}) for \"{}\" is spent. {}", task.title.trim(), takeover_text(self.backend.can_edit())));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, self.spent_state(), max, None, None, None, None, None).await;
            return None;
        }
        if let Some(why) = self.backend.limited(editor) {
            self.out.reports.push(limit_text(&task.title, &self.backend.actor_name(editor), &why));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, "limited", 0, None, Some(editor), self.backend.actor_model(editor), None, None).await;
            return None;
        }
        let problems = self.backend.path_problems(task);
        if !problems.is_empty() {
            self.out.reports.push(format!("Path check failed for \"{}\": the worker was NOT run and this is not counted as an attempt. {}. Resend the block with correct paths.", task.title.trim(), problems.join("; ")));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, "failed", 0, None, None, None, None, None).await;
            return None;
        }
        Some(self.next_attempt(task, editor, 0).await)
    }

    /// Counts one more attempt of the task and shows it on the board.
    async fn next_attempt(&mut self, task: &Delegation, editor: Actor, auto: usize) -> usize {
        let used = self.attempts.entry(key(&task.title)).or_insert(0);
        *used += 1;
        let attempt = *used;
        let fixing = attempt > 1 && auto == 0;
        self.mark(&task.title, if fixing { "fixing" } else { "coding" }, attempt, None, Some(editor), self.backend.actor_model(editor), None, None).await;
        attempt
    }

    /// What follows a worker run: the app's own retries of an empty or failed run, the report, and the hand-over to the checker.
    async fn settle(&mut self, task: Delegation, editor: Actor, mut attempt: usize, mut work: WorkerResult) {
        let title = key(&task.title);
        let max = limit(self.backend.can_escalate());
        let mut auto = 0;
        loop {
            if self.backend.cancelled() {
                return;
            }
            match auto_retry_reason(&work) {
                Some(why) if auto < MAX_AUTO_RETRIES && attempt < max && !work.error.as_deref().is_some_and(quota::is_limit) => {
                    auto += 1;
                    self.backend.say(format!("Tự giao lại {} ({auto}/{MAX_AUTO_RETRIES}): {}", self.backend.actor_name(editor), truncate_chars(&why, 160))).await;
                    attempt = self.next_attempt(&task, editor, auto).await;
                    work = self.backend.work(&task, attempt, Some(&why), editor).await;
                }
                _ => break,
            }
        }
        let checker = Actor::Tester;
        self.out.reports.push(report(&task, &self.backend.actor_name(editor), attempt, max, auto, &work));
        if let Some(why) = work.error.as_deref().filter(|e| auto_retry_reason(&work).is_some() && quota::is_limit(e)) {
            self.backend.note_limit(editor, why);
            self.out.reports.push(limit_text(&task.title, &self.backend.actor_name(editor), why));
            self.state.insert(title, State::Failed);
            self.mark(&task.title, "limited", attempt, None, Some(editor), self.backend.actor_model(editor), None, None).await;
            return;
        }
        if auto_retry_reason(&work).is_some() {
            self.state.insert(title, State::Failed);
            self.mark(&task.title, self.spent_state(), attempt, None, Some(editor), self.backend.actor_model(editor), None, None).await;
            return;
        }
        self.out.changed = true;
        if task.trivial {
            // Tiny low-risk change: no checker run; the coordinator is told to read the diff itself.
            self.out.reports.push(format!("Check skipped for \"{}\": you marked it trivial, so the checker did not check it. Read the diff above yourself.", task.title.trim()));
            self.state.insert(title, State::Clean);
            self.mark(&task.title, "done", attempt, None, Some(editor), self.backend.actor_model(editor), None, None).await;
            return;
        }
        let Some(jobs) = self.jobs.clone() else {
            self.state.insert(title, State::Clean);
            self.mark(&task.title, "done", attempt, None, Some(editor), self.backend.actor_model(editor), None, None).await;
            return;
        };
        self.backend.say(format!("{} kiểm tra: {} (lần {attempt})", self.backend.actor_name(checker), task.title.trim())).await;
        self.state.insert(title.clone(), State::Checking);
        self.mark(&task.title, "checking", attempt, None, Some(checker), self.backend.actor_model(checker), Some(checker), self.backend.actor_model(checker)).await;
        self.pending += 1;
        let shown = task.title.clone();
        if jobs.send((task, attempt, work)).is_err() {
            self.pending -= 1;
            self.state.insert(title, State::Clean);
            self.mark(&shown, "done", attempt, None, Some(editor), self.backend.actor_model(editor), Some(checker), self.backend.actor_model(checker)).await;
        }
    }

    async fn handle_check(&mut self, (task, attempt, check): Finished) {
        self.pending -= 1;
        if let Some(why) = check.error.as_deref().filter(|e| quota::is_limit(e)) {
            self.backend.note_limit(Actor::Tester, why);
        }
        self.backend.announce_check(&task, attempt, &check).await;
        self.out.reports.push(check_text(&task, attempt, &check));
        let title = key(&task.title);
        let shown = match check.verdict {
            Verdict::Pass => "PASS",
            Verdict::Fail => "FAIL",
            Verdict::Unclear => "UNCLEAR",
        };
        let checker = Actor::Tester;
        let checker_model = self.backend.actor_model(checker);
        let severe: Vec<&Issue> = check.issues.iter().filter(|i| i.is_severe()).collect();
        self.out.minor.extend(check.issues.iter().filter(|i| !i.is_severe()).map(|i| format!("{}: {} ({})", task.title.trim(), i.problem, i.file)));
        if check.verdict == Verdict::Fail && !severe.is_empty() {
            let max = limit(self.backend.can_escalate());
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
                    self.backend.say(format!("Thêm file người kiểm tra nêu vào task sửa: {}", added.join(", "))).await;
                }
                self.queue.push_back(fix);
                let next_editor = Actor::Worker(0);
                // The fix is queued, not started: it shows as waiting to be fixed until a worker takes it (then `work` marks it "fixing").
                self.mark(&task.title, "fixwait", attempt, Some(shown), Some(next_editor), self.backend.actor_model(next_editor), Some(checker), checker_model).await;
            } else {
                self.out.reports.push(format!("The attempt limit ({max}/{max}) for \"{}\" is spent and the check still fails. {}", task.title.trim(), takeover_text(self.backend.can_edit())));
                self.state.insert(title, State::Failed);
                self.mark(&task.title, self.spent_state(), attempt, Some(shown), None, None, Some(checker), checker_model).await;
            }
            return;
        }
        self.state.insert(title, State::Clean);
        self.mark(&task.title, "done", attempt, Some(shown), None, None, Some(checker), checker_model).await;
    }
}

/// Runs the tasks of one coordinator reply to the end. `attempts` (per task title) lives across rounds.
pub(in crate::providers) async fn run_round<B: Backend>(backend: &Arc<B>, tasks: Vec<Delegation>, attempts: &mut HashMap<String, usize>) -> RoundOutcome {
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
            rows.push(Row { title: task.title.trim().to_owned(), state: "waiting", attempt: 0, check: None, actor: None, model: None, checker: None, checker_model: None });
        }
    }
    let mut round = Round { backend, queue: tasks.into(), state: HashMap::new(), attempts, jobs, pending: 0, out: RoundOutcome::default(), rows };
    round.backend.board(&round.rows).await;
    while !backend.cancelled() {
        while let Ok(finished) = done_rx.try_recv() {
            round.handle_check(finished).await;
        }
        match next_ready(&mut round.queue, &round.state) {
            Next::Run(task) => {
                let mut group = vec![task];
                for _ in 1..backend.worker_slots().clamp(1, 3) {
                    let Some(partner) = take_partner(&mut round.queue, &round.state, &group) else {
                        break;
                    };
                    group.push(partner);
                }
                round.run_group(group).await;
            }
            Next::Skip(task, why) => {
                round.out.reports.push(format!("Task \"{}\" was skipped: {why}.", task.title.trim()));
                round.state.insert(key(&task.title), State::Failed);
                round.mark(&task.title, "skipped", 0, None, None, None, None, None).await;
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
