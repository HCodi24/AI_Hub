use super::check::{issue_files, Issue, Verdict};
use super::run_round;
use super::*;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::Notify;

fn task(title: &str, deps: &[&str]) -> Delegation {
    Delegation { title: title.into(), files: vec!["a.rs".into()], instructions: "do it".into(), acceptance: vec![], depends_on: deps.iter().map(|d| d.to_string()).collect(), trivial: false }
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
    /// Every worker run fails with a usage-limit error.
    out_of_limit: bool,
    boards: Mutex<Vec<Vec<Row>>>,
    /// Paths the fake project does not accept.
    bad_paths: Vec<String>,
    /// Title and files of every worker run.
    works: Mutex<Vec<(String, Vec<String>)>>,
    /// Configured slot count; zero in the default fixture uses one slot.
    worker_slots: usize,
    /// Worker runs in progress now and the most there ever were, with the slot each title ran on.
    active: std::sync::atomic::AtomicUsize,
    peak: std::sync::atomic::AtomicUsize,
    slots: Mutex<Vec<(String, Actor)>>,
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
    async fn work(&self, task: &Delegation, _attempt: usize, _feedback: Option<&str>, editor: Actor) -> WorkerResult {
        use std::sync::atomic::Ordering::SeqCst;
        self.log(format!("work:{}", task.title));
        self.works.lock().unwrap().push((task.title.clone(), task.files.clone()));
        self.slots.lock().unwrap().push((task.title.clone(), editor));
        self.peak.fetch_max(self.active.fetch_add(1, SeqCst) + 1, SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.active.fetch_sub(1, SeqCst);
        if self.hold.as_ref().is_some_and(|(_, other)| *other == task.title) {
            self.released.notify_one();
        }
        let diff = if self.empty.contains(&task.title) || self.out_of_limit { "" } else { "+x" };
        WorkerResult { summary: String::new(), diff: diff.into(), error: self.out_of_limit.then(|| "You hit your usage limit".to_owned()), refused: Vec::new() }
    }
    fn has_checker(&self) -> bool {
        self.checker
    }
    fn actor_name(&self, actor: Actor) -> String {
        match actor {
            Actor::Worker(slot) => format!("Worker {}", usize::from(slot) + 1),
            Actor::Tester => "Tester".to_owned(),
        }
    }
    fn worker_slots(&self) -> usize {
        self.worker_slots.max(1)
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
    fn actor_model(&self, actor: Actor) -> Option<String> {
        Some(match actor {
            Actor::Worker(_) => "worker-model",
            Actor::Tester => "tester-model",
        }.into())
    }
    async fn board(&self, rows: &[Row]) {
        self.boards.lock().unwrap().push(rows.to_vec());
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
    assert_eq!(out.reports.iter().filter(|r| r.starts_with("Kiểm tra độc lập")).count(), 2);
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
    // The worker has three attempts when a checker exists: the third failed check hands the task to the coordinator.
    assert_eq!(fake.lines().iter().filter(|l| l.starts_with("work:")).count(), 3);
    assert_eq!(attempts["a"], 3);
    assert!(out.reports.last().unwrap().contains("attempt limit (3/3)"));
    // The fake can edit, so the spent task is handed to the coordinator instead of showing a bare failure.
    assert_eq!(fake.boards.lock().unwrap().last().unwrap()[0].state, "takeover");
}

#[test]
fn attempt_limit_depends_on_escalation() {
    assert_eq!((limit(true), limit(false)), (3, MAX_ATTEMPTS));
}

#[tokio::test]
async fn two_independent_tasks_run_at_once_on_the_two_worker_slots() {
    let tasks = || vec![
        Delegation { files: vec!["src/a.rs".into()], ..task("A", &[]) },
        Delegation { files: vec!["web/b.ts".into()], ..task("B", &[]) },
    ];
    let (fake, _, _) = run(Fake { worker_slots: 2, checker: true, ..Default::default() }, tasks()).await;
    assert_eq!(fake.peak.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(*fake.slots.lock().unwrap(), [("A".to_string(), Actor::Worker(0)), ("B".to_string(), Actor::Worker(1))]);
    let boards = fake.boards.lock().unwrap();
    let coding = boards.iter().flat_map(|rows| rows.iter()).find(|row| row.title == "B" && row.state == "coding").unwrap();
    assert_eq!(coding.actor.as_deref(), Some("Worker 2"));
    drop(boards);
    // No second slot: one after another.
    let (fake, _, _) = run(Fake { checker: true, ..Default::default() }, tasks()).await;
    assert_eq!(fake.peak.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[tokio::test]
async fn tasks_that_share_a_file_or_depend_on_each_other_never_run_at_once() {
    let fake = || Fake { worker_slots: 2, checker: true, ..Default::default() };
    let folder = Delegation { files: vec!["src".into()], ..task("A", &[]) };
    let inside = Delegation { files: vec!["src/a.rs".into()], ..task("B", &[]) };
    let (peak, _) = { let (f, o, _) = run(fake(), vec![folder, inside]).await; (f.peak.load(std::sync::atomic::Ordering::SeqCst), o) };
    assert_eq!(peak, 1, "a folder covers the files inside it");
    let a = Delegation { files: vec!["x.rs".into()], ..task("A", &[]) };
    let b = Delegation { files: vec!["y.rs".into()], ..task("B", &["A"]) };
    let (f, _, _) = run(fake(), vec![a, b]).await;
    assert_eq!(f.peak.load(std::sync::atomic::Ordering::SeqCst), 1);
    let (f, _, _) = run(Fake { worker_slots: 2, ..Default::default() }, vec![task("A", &[]), Delegation { files: vec![], ..task("B", &[]) }]).await;
    assert_eq!(f.peak.load(std::sync::atomic::Ordering::SeqCst), 1, "a task without files cannot be proven independent");
}

#[tokio::test]
async fn three_independent_tasks_use_the_configured_number_of_slots() {
    for worker_slots in [1, 3] {
        let tasks = ["A", "B", "C"]
            .into_iter()
            .map(|title| Delegation {
                files: vec![format!("{title}.rs")],
                ..task(title, &[])
            })
            .collect();
        let (fake, _, _) = run(Fake { worker_slots, ..Default::default() }, tasks).await;
        let expected: Vec<_> = ["A", "B", "C"]
            .into_iter()
            .enumerate()
            .map(|(index, title)| (title.to_owned(), Actor::Worker((index % worker_slots) as u8)))
            .collect();
        assert_eq!(*fake.slots.lock().unwrap(), expected);
        assert_eq!(fake.peak.load(std::sync::atomic::Ordering::SeqCst), worker_slots);
    }
}

#[tokio::test]
async fn a_third_task_waits_when_it_overlaps_or_depends_on_the_second() {
    for (file, dependencies) in [
        ("src/b.rs", &[][..]),
        ("web/c.ts", &["B"][..]),
    ] {
        let tasks = vec![
            task("A", &[]),
            Delegation { files: vec!["src".into()], ..task("B", &[]) },
            Delegation { files: vec![file.into()], ..task("C", dependencies) },
        ];
        let (fake, _, _) = run(Fake { worker_slots: 3, ..Default::default() }, tasks).await;
        assert_eq!(fake.peak.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(
            *fake.slots.lock().unwrap(),
            [
                ("A".to_owned(), Actor::Worker(0)),
                ("B".to_owned(), Actor::Worker(1)),
                ("C".to_owned(), Actor::Worker(0)),
            ]
        );
    }
}

#[tokio::test]
async fn a_trivial_task_is_not_checked_and_the_coordinator_is_told_to_read_the_diff() {
    let fake = Fake { checker: true, ..Default::default() };
    let trivial = Delegation { trivial: true, ..task("A", &[]) };
    let (fake, out, _) = run(fake, vec![trivial, task("B", &[])]).await;
    let lines = fake.lines();
    assert!(!lines.contains(&"check:A".to_string()) && !lines.contains(&"announce:A".to_string()), "{lines:?}");
    assert!(lines.contains(&"check:B".to_string()), "other tasks are still checked: {lines:?}");
    assert!(out.reports.iter().any(|r| r.contains("Check skipped") && r.contains("\"A\"")));
    assert!(fake.boards.lock().unwrap().iter().any(|rows| rows.iter().any(|row| row.title == "A" && row.state == "done")));
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
    assert_eq!(boards[0].iter().map(|row| (row.title.as_str(), row.state)).collect::<Vec<_>>(), [("A", "waiting"), ("B", "waiting")]);
    for (title, state) in [("A", "coding"), ("A", "checking"), ("A", "fixwait"), ("A", "fixing"), ("A", "done"), ("B", "done")] {
        assert!(boards.iter().any(|rows| rows.iter().any(|row| row.title == title && row.state == state)), "missing {title}:{state}: {boards:?}");
    }
}

#[tokio::test]
async fn a_usage_limit_error_is_not_retried_and_is_not_a_takeover() {
    let fake = Fake { out_of_limit: true, ..Default::default() };
    let (fake, out, _) = run(fake, vec![task("A", &[])]).await;
    let works = fake.lines().iter().filter(|l| l.starts_with("work:A")).count();
    assert_eq!(works, 1, "no automatic retry: {:?}", fake.lines());
    assert!(fake.boards.lock().unwrap().iter().any(|rows| rows.iter().any(|row| row.state == "limited")));
    assert!(out.reports.iter().any(|r| r.contains("usage limit") && r.contains("NOT done")));
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
fn the_checker_answer_is_parsed_and_unreadable_answers_are_unclear() {        let ok = parse_check("Here: {\"verdict\":\"fail\",\"issues\":[{\"severity\":\"HIGH\",\"file\":\"a.rs\",\"problem\":\"p\",\"fix\":\"f\"},{\"problem\":\"\"}],\"notes\":\"n\"}");
    assert_eq!(ok.verdict, Verdict::Fail);
    assert_eq!(ok.issues.len(), 1, "an issue without a problem is dropped");
    assert!(ok.issues[0].is_severe() && ok.notes == "n");
    assert_eq!(parse_check("looks fine to me").verdict, Verdict::Unclear);
    assert_eq!(parse_check("{\"verdict\":\"maybe\"}").verdict, Verdict::Unclear);
    assert!(!Issue { severity: "low".into(), file: String::new(), problem: "x".into(), fix: String::new() }.is_severe());
}
#[test]
fn worker_slots_keep_compatible_session_keys() {
    use crate::providers::delegate::slot_key;

    assert_eq!(slot_key("provider", 0), "provider");
    assert_eq!(slot_key("provider", 1), "provider#2");
    assert_eq!(slot_key("provider", 2), "provider#3");
    assert_eq!(Fake::default().actor_name(Actor::Worker(1)), "Worker 2");
    assert_eq!(Fake::default().actor_name(Actor::Worker(2)), "Worker 3");
}
