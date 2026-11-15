//! MutiS High chat: Codex coordinates, the app runs the DeepSeek worker and hands the real report back to Codex.
//!
//! Codex ends a turn with fenced `mutis-delegate` JSON blocks (see `core::rule::mutis_high_delegation`). The app runs
//! each block on the worker, then resumes the same Codex thread with the worker's report. The app, not Codex, counts
//! the attempts per task and tells Codex to take a task over once the limit is spent.
use super::schedule::{check_text, parse_check, run_round, Backend, CheckResult, Row};
use super::{for_kind, ProviderEvent, ProviderRequest};
use crate::chat::{ChatMessage, Effort, Permission};
use crate::config::{MutisConfig, ProviderConfig};
use crate::core::rule::mutis_checker;
use crate::core::text::truncate_chars;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const MAX_ATTEMPTS: usize = 4;
/// Empty or failed worker runs the app repeats by itself (they still count as attempts) before the coordinator hears about them.
pub(super) const MAX_AUTO_RETRIES: usize = 2;
/// Guard against a Codex that keeps delegating: the number of Codex turns in one chat reply.
const MAX_ROUNDS: usize = 12;
const FENCE: &str = "```mutis-delegate";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Delegation {
    pub title: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub instructions: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
    /// Titles of tasks whose result this one needs; it starts only after they passed their check. Independent tasks start at once.
    #[serde(default)]
    pub depends_on: Vec<String>,
}

/// Valid delegations in the text, plus a message for every block that did not parse.
pub fn extract(text: &str) -> (Vec<Delegation>, Vec<String>) {
    let (mut tasks, mut errors) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(start) = rest.find(FENCE) {
        let body = &rest[start + FENCE.len()..];
        let end = body.find("```").unwrap_or(body.len());
        match serde_json::from_str::<Delegation>(body[..end].trim()) {
            Ok(task) if !task.title.trim().is_empty() && !task.instructions.trim().is_empty() => tasks.push(task),
            Ok(_) => errors.push("a mutis-delegate block has an empty title or instructions".into()),
            Err(error) => errors.push(format!("a mutis-delegate block is not valid JSON ({error})")),
        }
        rest = &body[end..];
    }
    (tasks, errors)
}

/// Paths Codex named must be real before a worker run is spent: an existing file, or a new file in an existing folder.
pub fn check_files(folder: &str, files: &[String]) -> Vec<String> {
    if files.is_empty() {
        return vec!["the files list is empty: name the exact files".into()];
    }
    let root = std::path::Path::new(folder);
    files.iter().filter_map(|file| {
        let relative = file.replace('\\', "/");
        if relative.starts_with('/') || relative.as_bytes().get(1) == Some(&b':') || relative.split('/').any(|part| part == "..") {
            return Some(format!("`{file}` must be a relative path inside the project"));
        }
        let path = root.join(&relative);
        let parent_exists = path.parent().is_some_and(|parent| parent.is_dir());
        (!path.is_file() && !parent_exists).then(|| format!("`{file}` does not exist and neither does its folder"))
    }).collect()
}

const PRELOAD_MAX_LINES: usize = 150;

/// Current text of the small listed files, as `FILE:` blocks (the worker then skips those reads). Larger files, binary
/// files and files that contain a code fence stay for the worker to read by range.
fn preload(folder: &str, files: &[String]) -> String {
    files.iter().filter_map(|file| {
        let text = std::fs::read_to_string(std::path::Path::new(folder).join(file.replace('\\', "/"))).ok()?;
        (text.lines().count() <= PRELOAD_MAX_LINES && !text.contains("```")).then(|| format!("FILE: {file}\n{}\n```\n", text.trim_end()))
    }).collect()
}

fn worker_prompt(mutis: &MutisConfig, task: &Delegation, preloaded: &str, feedback: Option<&str>) -> String {
    let mut parts = Vec::new();
    if mutis.enabled && !mutis.worker_rules.trim().is_empty() {
        parts.push(mutis.worker_rules.clone());
    }
    parts.push(format!("Task: {}", task.title.trim()));
    if !task.files.is_empty() {
        parts.push(format!("Files: {}", task.files.join(", ")));
    }
    parts.push(format!("Instructions:\n{}", task.instructions.trim()));
    if !task.acceptance.is_empty() {
        parts.push(format!("Acceptance criteria:\n{}", task.acceptance.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n")));
    }
    if !preloaded.is_empty() {
        parts.push(format!("Current content of the small listed files (already read for you; do not read them again):\n{preloaded}"));
    }
    if let Some(why) = feedback {
        parts.push(format!("The previous attempt failed: {why}. Do not investigate again: make the edit now, only in the listed files."));
    }
    parts.push("Make the edits yourself with the file tools; investigating without editing is a failed task. End with the files you changed and anything left undone.".into());
    parts.join("\n")
}

pub(super) struct WorkerResult {
    pub(super) summary: String,
    pub(super) diff: String,
    pub(super) error: Option<String>,
    /// Edits the scope refused (outside the listed files).
    pub(super) refused: Vec<String>,
}

/// Why the app can tell, without the coordinator, that a worker run produced nothing usable. None = changed something.
pub(super) fn auto_retry_reason(result: &WorkerResult) -> Option<String> {
    if !result.diff.trim().is_empty() {
        return None;
    }
    let mut why = result.error.as_deref().map_or("it changed no file".to_owned(), |e| format!("it failed with: {}", truncate_chars(e, 300)));
    if !result.refused.is_empty() {
        why.push_str(&format!("; edits refused as outside the listed files: {}", truncate_chars(&result.refused.join("; "), 300)));
    }
    Some(why)
}

pub(super) fn report(task: &Delegation, attempt: usize, auto: usize, result: &WorkerResult) -> String {
    let auto_note = if auto > 0 { format!("The app already re-ran the worker {auto} time(s) before this report because a run changed no file or failed.\n") } else { String::new() };
    let changed = if result.diff.trim().is_empty() {
        "NONE - the worker did not change any file".to_owned()
    } else {
        format!("yes, diff:\n{}", truncate_chars(&result.diff, 6000))
    };
    let status = result.error.as_deref().map_or("finished".to_owned(), |e| format!("ERROR: {e}"));
    format!(
        "DeepSeek worker report for \"{}\" (attempt {attempt}/{MAX_ATTEMPTS}, {} attempts left for this task):\n{auto_note}Status: {status}\nFiles changed: {changed}\nWorker summary:\n{}",
        task.title.trim(),
        MAX_ATTEMPTS - attempt,
        truncate_chars(&result.summary, 3000),
    )
}

/// Closes every report sent to the coordinator.
const REVIEW_INSTRUCTION: &str = "Review every result above against your acceptance criteria and the diffs. If a task passes, say so and continue. If a task fails, answer with another mutis-delegate block with the SAME title and concrete corrections; the attempts left are shown in its report.";

/// A new chronological turn: the browser and the saved record start a fresh assistant message for this provider.
fn turn(provider_kind: &str, model: &str, name: &str) -> ProviderEvent {
    ProviderEvent {
        name: "turn",
        data: serde_json::json!({ "provider_kind": provider_kind, "model": model, "provider_name": name }),
    }
}

fn is_token_line(line: &str) -> bool {
    line.starts_with("Token:")
}

/// A worker log line as shown in the chat; token lines stay unprefixed because the token table and task_end only match them at the start.
fn worker_log(line: &str) -> String {
    if is_token_line(line) { line.to_owned() } else { format!("DeepSeek: {line}") }
}

fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        debate_id: None, plan_id: None, role: role.into(), content, attachments: Vec::new(), reasoning: String::new(),
        logs: Vec::new(), diff: String::new(), files: Vec::new(), provider: String::new(), model: String::new(), error: None,
    }
}

pub struct DelegateProvider {
    pub worker: ProviderConfig,
    pub model: String,
    /// The independent checker (a Codex source, its model and effort), when the option is on and configured.
    pub checker: Option<(ProviderConfig, String, Effort)>,
    /// Shown once in the chat when the option is on but the checker cannot run (what to set in the High section).
    pub checker_note: Option<String>,
}

/// What one round needs to talk to the worker and the checker; shared by the round loop and the checker's task.
struct Live {
    worker: ProviderConfig,
    model: String,
    folder: String,
    mutis: MutisConfig,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
    checker: Option<(ProviderConfig, String, Effort)>,
    /// The checker's own Codex thread: resumed for every check, kept in the chat record between messages.
    checker_session: Mutex<Option<String>>,
    /// The checker's last "Token:" log line, held until `announce_check` shows it in the Luna turn.
    checker_tokens: Mutex<Option<String>>,
    /// Whether the coordinator may edit files, i.e. can take a task over when the attempts run out.
    can_edit: bool,
}

#[async_trait::async_trait]
impl Backend for Live {
    async fn work(&self, task: &Delegation, attempt: usize, feedback: Option<&str>) -> WorkerResult {
        let events = &self.events;
        if self.cancel.is_cancelled() || events.send(turn("deepseek", &self.model, "DeepSeek")).await.is_err() {
            self.cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        }
        let _ = events.send(ProviderEvent::text("log", format!("Giao DeepSeek: {} (lần {attempt}/{MAX_ATTEMPTS})", task.title.trim()))).await;
        let mut provider = self.worker.clone();
        provider.kind = "deepseek_agent".into();
        provider.agent_capable = true;
        let worker_request = ProviderRequest {
            provider, model: self.model.clone(), folder: self.folder.clone(), permission: Permission::Write, effort: Effort::Default,
            messages: vec![message("user", worker_prompt(&self.mutis, task, &preload(&self.folder, &task.files), feedback))], session_id: None, mutis: self.mutis.clone(), mcp: Vec::new(),
            write_scope: task.files.clone(),
            checker_session_id: None,
        };
        let (sender, mut receiver) = mpsc::channel(32);
        let (inner_cancel, run) = (self.cancel.clone(), for_kind("deepseek_agent"));
        let handle = tokio::spawn(async move {
            match run {
                Ok(provider) => provider.run(worker_request, sender, inner_cancel).await,
                Err(error) => Err(error),
            }
        });
        let mut result = WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => {
                    result.summary.push_str(event.data["text"].as_str().unwrap_or(""));
                    let _ = events.send(event).await;
                }
                "reasoning" => {
                    let _ = events.send(event).await;
                }
                "diff" => {
                    result.diff.push_str(event.data["text"].as_str().unwrap_or(""));
                    let _ = events.send(event).await;
                }
                "log" => {
                    let line = event.data["text"].as_str().unwrap_or("");
                    // The agent logs "Từ chối: <path> (ngoài phạm vi được giao)" when the scope stops an edit.
                    if line.contains("ngoài phạm vi") {
                        result.refused.push(line.trim_start_matches("Từ chối: ").to_owned());
                    }
                    // The token table and task_end only match lines that start with "Token:", so those stay unprefixed.
                    let line = worker_log(line);
                    let _ = events.send(ProviderEvent::text("log", line)).await;
                }
                "error" => {
                    result.error = event.data["text"].as_str().map(str::to_owned);
                    // A worker failure stays visible in its own turn; the coordinator still gets it in the report.
                    let _ = events.send(event).await;
                }
                _ => {}
            }
        }
        let failure = match handle.await {
            Ok(Err(error)) => Some(error.to_string()),
            Err(error) => Some(error.to_string()),
            Ok(Ok(())) => None,
        };
        if let Some(error) = failure {
            if result.error.as_deref() != Some(error.as_str()) {
                let _ = events.send(ProviderEvent::text("error", error.clone())).await;
            }
            result.error = Some(error);
        }
        result
    }

    fn has_checker(&self) -> bool {
        self.checker.is_some()
    }

    async fn check(&self, task: &Delegation, _attempt: usize, work: &WorkerResult) -> CheckResult {
        let Some((provider, model, effort)) = self.checker.clone() else { return CheckResult::unclear("no checker is configured") };
        let session = self.checker_session.lock().unwrap().clone();
        let request = ProviderRequest {
            provider, model, folder: self.folder.clone(), permission: Permission::Read, effort,
            messages: vec![message("user", mutis_checker::prompt(&task.title, &task.files, &task.acceptance, &work.summary, &work.diff))],
            session_id: session, mutis: self.mutis.clone(), mcp: Vec::new(), write_scope: Vec::new(), checker_session_id: None,
        };
        let inner = match for_kind("codex_cli") {
            Ok(inner) => inner,
            Err(error) => return CheckResult::unclear(&error.to_string()),
        };
        let (sender, mut receiver) = mpsc::channel(32);
        let cancel = self.cancel.clone();
        let handle = tokio::spawn(async move { inner.run(request, sender, cancel).await });
        let (mut text, mut error) = (String::new(), None::<String>);
        // Nothing the checker streams reaches the chat; the round loop announces the finished verdict.
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => text.push_str(event.data["text"].as_str().unwrap_or("")),
                "session" => {
                    if let Some(id) = event.data["session_id"].as_str() {
                        *self.checker_session.lock().unwrap() = Some(id.to_owned());
                    }
                }
                "log" => {
                    // Keep only the token line so Luna's usage reaches the token table.
                    if let Some(line) = event.data["text"].as_str().filter(|line| is_token_line(line)) {
                        *self.checker_tokens.lock().unwrap() = Some(line.to_owned());
                    }
                }
                "error" => error = event.data["text"].as_str().map(str::to_owned),
                _ => {}
            }
        }
        match handle.await {
            Ok(Err(failure)) => error = Some(failure.to_string()),
            Err(failure) => error = Some(failure.to_string()),
            Ok(Ok(())) => {}
        }
        match error {
            Some(error) if text.trim().is_empty() => CheckResult::unclear(&error),
            _ => parse_check(&text),
        }
    }

    async fn say(&self, line: String) {
        let _ = self.events.send(ProviderEvent::text("log", line)).await;
    }

    async fn announce_check(&self, task: &Delegation, attempt: usize, check: &CheckResult) {
        let model = self.checker.as_ref().map_or("", |(_, model, _)| model.as_str());
        if self.events.send(turn("codex_cli", model, "Luna")).await.is_err() {
            self.cancel.cancel();
            return;
        }
        let session = self.checker_session.lock().unwrap().clone();
        if let Some(id) = session {
            let _ = self.events.send(ProviderEvent { name: "checker_session", data: serde_json::json!({ "session_id": id }) }).await;
        }
        let _ = self.events.send(ProviderEvent::text("delta", format!("**Luna kiểm tra**\n\n{}\n", check_text(task, attempt, check)))).await;
        let tokens = self.checker_tokens.lock().unwrap().take();
        if let Some(line) = tokens {
            let _ = self.events.send(ProviderEvent::text("log", line)).await;
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn can_edit(&self) -> bool {
        self.can_edit
    }

    async fn board(&self, rows: &[Row]) {
        let tasks: Vec<_> = rows.iter().map(|row| serde_json::json!({ "title": row.title, "state": row.state, "attempt": row.attempt, "check": row.check })).collect();
        let _ = self.events.send(ProviderEvent { name: "tasks", data: serde_json::json!({ "tasks": tasks }) }).await;
    }

    fn path_problems(&self, task: &Delegation) -> Vec<String> {
        check_files(&self.folder, &task.files)
    }
}

impl DelegateProvider {
    /// One Codex turn: every event goes to the chat; returns the reply text and the native session id.
    async fn codex_turn(&self, request: &ProviderRequest, events: &mpsc::Sender<ProviderEvent>, cancel: &CancellationToken) -> Result<(String, Option<String>)> {
        // The coordinator gets its own turn before the call, so its text never mixes with a worker turn.
        if cancel.is_cancelled() { return Ok((String::new(), None)); }
        if events.send(turn("codex_cli", &request.model, "Codex")).await.is_err() {
            cancel.cancel();
            return Ok((String::new(), None));
        }
        let (sender, mut receiver) = mpsc::channel(32);
        let inner = for_kind("codex_cli")?;
        let (inner_request, inner_cancel) = (request.clone(), cancel.clone());
        let handle = tokio::spawn(async move { inner.run(inner_request, sender, inner_cancel).await });
        let (mut text, mut session) = (String::new(), None);
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => text.push_str(event.data["text"].as_str().unwrap_or("")),
                "session" => session = event.data["session_id"].as_str().map(str::to_owned),
                _ => {}
            }
            if events.send(event).await.is_err() {
                cancel.cancel();
                break;
            }
        }
        handle.await.map_err(|e| anyhow!(e))??;
        Ok((text, session))
    }

    /// The project's own checks, run once after the whole round (never beside a worker that is still editing).
    async fn project_checks(&self, request: &ProviderRequest, events: &mpsc::Sender<ProviderEvent>) -> Option<String> {
        if !request.mutis.effective(request.mutis.verify_gate) {
            return None;
        }
        let folder = std::path::Path::new(&request.folder);
        let command = crate::verify::command_for(&request.mutis, folder);
        let _ = events.send(ProviderEvent::text("log", "Kiểm tra dự án sau khi các task xong…".to_owned())).await;
        let (sender, mut lines) = mpsc::channel::<String>(64);
        // `verify::run` stops its command when the receiver is gone, so the lines are drained, not dropped.
        let drain = tokio::spawn(async move { while lines.recv().await.is_some() {} });
        let report = crate::verify::run(folder, &command, &request.mutis.verify_ignore, sender).await;
        let _ = drain.await;
        let state = if report.unverified { "NOT VERIFIED (no check command for this project)" } else if report.ok { "OK" } else { "FAILED" };
        let _ = events.send(ProviderEvent::text("log", format!("Kiểm tra dự án: {state}"))).await;
        Some(format!("Project checks, run once after all tasks of this round: {state}\n{}\n\n", report.summary_lines.join("\n")))
    }
}

#[async_trait::async_trait]
impl super::Provider for DelegateProvider {
    async fn run(&self, mut request: ProviderRequest, events: mpsc::Sender<ProviderEvent>, cancel: CancellationToken) -> Result<()> {
        let live = Arc::new(Live {
            worker: self.worker.clone(), model: self.model.clone(), folder: request.folder.clone(), mutis: request.mutis.clone(),
            events: events.clone(), cancel: cancel.clone(), checker: self.checker.clone(),
            checker_session: Mutex::new(request.checker_session_id.clone()),
            checker_tokens: Mutex::new(None),
            can_edit: request.permission != Permission::Read,
        });
        let mut attempts: HashMap<String, usize> = HashMap::new();
        let mut note = self.checker_note.clone();
        for _ in 0..MAX_ROUNDS {
            let (text, session) = self.codex_turn(&request, &events, &cancel).await?;
            // The option is on but the checker cannot run: say so once, in the coordinator's activity, instead of silently skipping it.
            if let Some(note) = note.take() {
                let _ = events.send(ProviderEvent::text("log", note)).await;
            }
            if session.is_some() {
                request.session_id = session;
            }
            let (tasks, errors) = extract(&text);
            if cancel.is_cancelled() || (tasks.is_empty() && errors.is_empty()) {
                return Ok(());
            }
            let mut reply = errors.iter().map(|e| format!("Problem: {e}. Resend it as valid JSON.\n")).collect::<String>();
            let outcome = run_round(&live, tasks, &mut attempts).await;
            if cancel.is_cancelled() {
                return Ok(());
            }
            for item in &outcome.reports {
                reply.push_str(item);
                reply.push_str("\n\n");
            }
            if !outcome.minor.is_empty() {
                reply.push_str(&format!("Minor findings of the checker (not sent to the worker; decide whether any matters):\n- {}\n\n", outcome.minor.join("\n- ")));
            }
            if outcome.changed {
                if let Some(checks) = self.project_checks(&request, &events).await {
                    reply.push_str(&checks);
                }
            }
            reply.push_str(REVIEW_INSTRUCTION);
            request.messages.push(message("assistant", text));
            request.messages.push(message("user", reply));
        }
        let _ = events.send(ProviderEvent::text("log", format!("Dừng giao việc: quá {MAX_ROUNDS} vòng Codex trong một lượt"))).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_token_lines_stay_unprefixed_so_the_token_table_counts_them() {
        assert_eq!(worker_log("Token: vào 10 (cache 2), ra 3 (suy luận 0)"), "Token: vào 10 (cache 2), ra 3 (suy luận 0)");
        assert_eq!(worker_log("Sửa a.rs"), "DeepSeek: Sửa a.rs");
        assert!(is_token_line("Token: vào 1, ra 2") && !is_token_line("DeepSeek: Token: vào 1, ra 2"));
    }

    #[test]
    fn extracts_valid_blocks_and_reports_broken_ones() {
        let text = "Plan.\n```mutis-delegate\n{\"title\":\"A\",\"files\":[\"src/a.rs\"],\"instructions\":\"do a\",\"acceptance\":[\"x\"]}\n```\nand\n```mutis-delegate\n{not json}\n```\n```mutis-delegate\n{\"title\":\"B\",\"instructions\":\"do b\"}\n```";
        let (tasks, errors) = extract(text);
        assert_eq!(tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(tasks[0].files, ["src/a.rs"]);
        assert_eq!(errors.len(), 1);
        assert!(extract("no blocks here").0.is_empty());
        assert_eq!(extract("```mutis-delegate\n{\"title\":\" \",\"instructions\":\"x\"}\n```").1.len(), 1);
    }

    #[test]
    fn report_flags_a_worker_that_changed_nothing() {
        let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![] };
        let idle = WorkerResult { summary: "I only read files".into(), diff: String::new(), error: None, refused: Vec::new() };
        let text = report(&task, 2, 1, &idle);
        assert!(text.contains("attempt 2/4") && text.contains("NONE") && text.contains("2 attempts left"));
        let edited = WorkerResult { summary: "done".into(), diff: "+ new line".into(), error: Some("boom".into()), refused: Vec::new() };
        let text = report(&task, 4, 0, &edited);
        assert!(text.contains("ERROR: boom") && text.contains("+ new line") && text.contains("0 attempts left"));
    }

    #[test]
    fn app_retries_only_runs_that_changed_nothing() {
        let run = |diff: &str, error: Option<&str>, refused: &[&str]| WorkerResult {
            summary: String::new(), diff: diff.into(), error: error.map(String::from), refused: refused.iter().map(|s| s.to_string()).collect(),
        };
        assert!(auto_retry_reason(&run("+x", None, &[])).is_none());
        assert!(auto_retry_reason(&run("+x", Some("late error"), &[])).is_none(), "a partial change goes to the coordinator");
        assert_eq!(auto_retry_reason(&run("", None, &[])).as_deref(), Some("it changed no file"));
        let why = auto_retry_reason(&run("", Some("boom"), &["a.rs (ngoài phạm vi được giao)"])).unwrap();
        assert!(why.contains("failed with: boom") && why.contains("a.rs"));
    }

    #[test]
    fn check_files_accepts_real_files_and_new_files_in_real_folders_only() {
        let root = std::env::temp_dir().join(format!("ai-hub-check-{}", std::process::id()));
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/a.ts"), "x").unwrap();
        let folder = root.to_str().unwrap();
        let ok = |files: &[&str]| check_files(folder, &files.iter().map(|f| f.to_string()).collect::<Vec<_>>()).is_empty();
        assert!(ok(&["web/a.ts", "web/new.ts"]));
        assert!(!ok(&["nope/new.ts"]) && !ok(&["../x.ts"]) && !ok(&["C:/x.ts"]) && !ok(&[]));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_prompt_carries_the_task_files_and_criteria() {
        let task = Delegation { title: "T".into(), files: vec!["a.rs".into(), "b.rs".into()], instructions: "change x".into(), acceptance: vec!["builds".into()], depends_on: vec![] };
        let prompt = worker_prompt(&MutisConfig::default(), &task, "FILE: a.rs\nfn a() {}\n```\n", None);
        assert!(prompt.contains("Task: T") && prompt.contains("Files: a.rs, b.rs") && prompt.contains("- builds"));
        assert!(prompt.contains("FILE: a.rs") && !worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("already read"));
        assert!(worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("previous attempt failed: it changed no file"));
    }

    #[test]
    fn preload_keeps_small_plain_files_only() {
        let root = std::env::temp_dir().join(format!("ai-hub-preload-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("small.ts"), "const a = 1;\n").unwrap();
        std::fs::write(root.join("big.ts"), "x\n".repeat(PRELOAD_MAX_LINES + 1)).unwrap();
        std::fs::write(root.join("fenced.md"), "```\ncode\n```\n").unwrap();
        let files = ["small.ts", "big.ts", "fenced.md", "missing.ts"].map(String::from);
        let text = preload(root.to_str().unwrap(), &files);
        assert!(text.contains("FILE: small.ts\nconst a = 1;") && !text.contains("big.ts") && !text.contains("fenced.md") && !text.contains("missing"));
        assert_eq!(crate::providers::deepseek_agent::parse_edit_only(&text), vec![("small.ts".to_owned(), "const a = 1;".to_owned())]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
