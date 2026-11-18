//! MutiS High chat: Codex coordinates, the app runs the DeepSeek worker and hands the real report back to Codex.
//!
//! Codex ends a turn with fenced `mutis-delegate` JSON blocks (see `core::rule::mutis_high_delegation`). The app runs
//! each block on the worker, then resumes the same Codex thread with the worker's report. The app, not Codex, counts
//! the attempts per task and tells Codex to take a task over once the limit is spent.
use super::schedule::{check_text, limit, parse_check, run_round, Actor, Backend, CheckResult, Row};
use super::{for_kind, ProviderEvent, ProviderRequest};
use crate::chat::{ChatMessage, Effort, Permission};
use crate::config::{McpServerConfig, MutisConfig, ProviderConfig};
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
    /// Only with a DeepSeek worker: the coordinator marks a big task it will do itself, so the app does not delegate it.
    #[serde(default)]
    pub hard: bool,
    /// The coordinator marks a tiny low-risk change (a few lines of styling or text): Tester does not check it, the coordinator reads the diff.
    #[serde(default)]
    pub trivial: bool,
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

/// Style and markup files by extension. Component files (.tsx, .jsx, .vue, .svelte) are NOT listed: the same file holds layout
/// changes and plain data wiring, so the coordinator judges those (see `NO_UI_NOTE`).
fn is_ui_file(path: &str) -> bool {
    const UI: [&str; 5] = ["css", "scss", "sass", "less", "html"];
    path.rsplit_once('.').is_some_and(|(_, ext)| UI.iter().any(|ui| ext.eq_ignore_ascii_case(ui)))
}

/// With the "DeepSeek may do UI" switch off, a task with UI files never runs on DeepSeek: it goes back to the coordinator.
/// Returns the tasks to run and the notes for the coordinator.
fn guard_ui(tasks: Vec<Delegation>, ui_allowed: bool) -> (Vec<Delegation>, Vec<String>) {
    if ui_allowed {
        return (tasks, Vec::new());
    }
    let (mut kept, mut notes, mut dropped) = (Vec::new(), Vec::new(), Vec::<String>::new());
    let key = |title: &str| title.trim().to_lowercase();
    for task in tasks {
        let blocked_by = task.depends_on.iter().find(|dep| dropped.contains(&key(dep))).cloned();
        if let Some(dep) = blocked_by {
            notes.push(format!("Task \"{}\" was not run: it depends on \"{dep}\", which you must implement yourself first (UI is off for DeepSeek). Resend it afterwards.", task.title.trim()));
            dropped.push(key(&task.title));
        } else if !task.hard && task.files.iter().any(|file| is_ui_file(file)) {
            notes.push(format!("Task \"{}\" has UI files and DeepSeek may not do UI (setting). Implement it yourself.", task.title.trim()));
            dropped.push(key(&task.title));
        } else {
            kept.push(task);
        }
    }
    (kept, notes)
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
    // Applied at runtime, so existing configurations and sessions get the language rule too.
    parts.push(crate::core::rule::mutis_high_delegation::VIETNAMESE_RULE.to_owned());
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

/// The report the coordinator reads. The headings are Vietnamese (the app's own prose); the actor name, the task title,
/// the attempt numbers, the diff and the original error text stay verbatim.
pub(super) fn report(task: &Delegation, who: &str, attempt: usize, max: usize, auto: usize, result: &WorkerResult) -> String {
    let auto_note = if auto > 0 {
        format!("Ứng dụng đã tự chạy lại worker {auto} lần trước báo cáo này vì một lần chạy không đổi file nào hoặc bị lỗi.\n")
    } else {
        String::new()
    };
    let changed = if result.diff.trim().is_empty() {
        "KHÔNG - worker không thay đổi file nào".to_owned()
    } else {
        format!("có, diff:\n{}", truncate_chars(&result.diff, 6000))
    };
    let status = result.error.as_deref().map_or("đã xong".to_owned(), |e| format!("LỖI: {e}"));
    format!(
        "Báo cáo của worker {who} cho \"{}\" (lần {attempt}/{max}, còn {} lần cho task này):\n{auto_note}Trạng thái: {status}\nCác file đã đổi: {changed}\nTóm tắt của worker:\n{}",
        task.title.trim(),
        max.saturating_sub(attempt),
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

/// The input tokens of a `Token: vào N (cache C), ra M` line.
fn input_tokens(line: &str) -> Option<u64> {
    let digits: String = line.strip_prefix("Token: vào ")?.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// A worker log line as shown in the chat; token lines stay unprefixed because the token table and task_end only match them at the start.
fn worker_log(line: &str, label: &str) -> String {
    if is_token_line(line) { line.to_owned() } else { format!("{label}: {line}") }
}

fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        debate_id: None, plan_id: None, role: role.into(), content, attachments: Vec::new(), reasoning: String::new(),
        logs: Vec::new(), diff: String::new(), files: Vec::new(), tasks: None, provider: String::new(), model: String::new(), error: None,
    }
}

/// Stable identity of one check: the task title plus its attempt number. Attempts are counted per task across
/// rounds, so the same check keeps one id in `check` and `announce_check` and no two checks collide.
fn check_id(task: &Delegation, attempt: usize) -> String {
    format!("{}#{}", task.title.trim().to_lowercase(), attempt)
}

/// A live checker card event: shown beside the conversation while Tester runs, never appended to a chat message.
fn checker_activity(id: &str, phase: &str, task: &Delegation, attempt: usize, model: &str, text: &str) -> ProviderEvent {
    ProviderEvent {
        name: "checker_activity",
        data: serde_json::json!({ "checker_activity": {
            "id": id, "phase": phase, "title": task.title.trim(), "attempt": attempt,
            "model": model, "provider_name": "Người kiểm tra", "text": text,
        } }),
    }
}

/// A checker log line worth showing on the live card: tool activity and commands, never the raw verdict JSON.
fn checker_progress(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('{') || line.starts_with('[') || line.starts_with("```") {
        return None;
    }
    Some(truncate_chars(line, 200))
}

pub struct DelegateProvider {
    /// The source that runs the tasks: an API agent (OpenAI-compatible) or a CLI (Codex, Claude), whichever is active.
    pub worker: ProviderConfig,
    pub model: String,
    /// Reasoning effort of the worker role; used when the worker is a CLI.
    pub effort: Effort,
    /// The independent checker (a Codex source, its model and effort), when the option is on and configured.
    pub checker: Option<(ProviderConfig, String, Effort)>,
    /// Shown once in the chat when the option is on but the checker cannot run (what to set in the High section).
    pub checker_note: Option<String>,
    pub chat_id: String,
    /// The CLI worker's thread kept in the chat record from an earlier message; None = start a new one.
    pub worker_session: Option<String>,
    /// The same for Worker 2, the second slot of the same worker.
    pub worker2_session: Option<String>,
}

/// Key under which Worker 2's thread is kept in the chat record, next to the worker's own (`worker_id`).
pub fn worker2_key(worker_id: &str) -> String {
    format!("{worker_id}#2")
}

/// What one round needs to talk to the worker and the checker; shared by the round loop and the checker's task.
struct Live {
    worker: ProviderConfig,
    model: String,
    effort: Effort,
    /// The worker's own CLI thread (only when the worker is a CLI source); kept for every task of this reply.
    worker_cli_session: Mutex<Option<String>>,
    /// Input tokens of the worker thread's last model call (a Codex worker only): its current size, kept in the chat record.
    worker_cli_tokens: std::sync::atomic::AtomicU64,
    /// Worker 2's own CLI thread and its size: the same worker, a second conversation that runs beside the first.
    worker2_cli_session: Mutex<Option<String>>,
    worker2_cli_tokens: std::sync::atomic::AtomicU64,
    folder: String,
    mutis: MutisConfig,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
    checker: Option<(ProviderConfig, String, Effort)>,
    /// Key of the worker's conversation: one per chat, so every task and retry continues what the worker already read.
    worker_session: String,
    /// The checker's own Codex thread: resumed for every check, kept in the chat record between messages.
    checker_session: Mutex<Option<String>>,
    /// A Codex thread runs one turn at a time: the checker checks one task after another.
    tester_busy: tokio::sync::Mutex<()>,
    /// The checker's last "Token:" log line, held until `announce_check` shows it in the Tester turn.
    checker_tokens: Mutex<Option<String>>,
    /// Whether the coordinator may edit files, i.e. can take a task over when the attempts run out.
    can_edit: bool,
    /// The chat's permission: the coordinator and the checker run with it.
    permission: Permission,
    /// CodeGraph's MCP server as the lead got it (indexed project, option on); None = no role gets it.
    codegraph: Option<McpServerConfig>,
}

/// Said to every role that gets the CodeGraph tool, so it searches the index before reading files one by one.
const CODEGRAPH_NOTE: &str = "\nThe codegraph MCP tool is attached: use codegraph_explore to find and read code before opening files one by one.";

fn cli_mcp(codegraph: Option<&McpServerConfig>, kind: &str) -> Vec<McpServerConfig> {
    if matches!(kind, "codex_cli" | "claude_cli") { codegraph.cloned().into_iter().collect() } else { Vec::new() }
}

const CLI_WORK_NOTE: &str = concat!(
    "
You work directly in the project folder. Edit only what the task needs. ",
    "After editing run only quick checks of the files you changed (format check, lint, type check). ",
    "Do not set up a build environment, and do not run end-to-end tests or the whole project check: a separate checker does that."
);

impl Live {
    /// The tools of one delegated role: only the CLI sources can call an MCP server, and none of them gets the lead's own servers.
    fn mcp_for(&self, kind: &str) -> Vec<McpServerConfig> {
        cli_mcp(self.codegraph.as_ref(), kind)
    }

    /// The prompt of a CLI role, with the CodeGraph hint when the tool is attached.
    fn with_codegraph_note(&self, prompt: String, mcp: &[McpServerConfig]) -> String {
        if mcp.is_empty() { prompt } else { prompt + CODEGRAPH_NOTE }
    }

    /// Whether the worker runs as a CLI thread (Codex, Claude) instead of the API agent loop.
    fn worker_is_cli(&self) -> bool {
        matches!(self.worker.kind.as_str(), "codex_cli" | "claude_cli")
    }

    fn worker_is_deepseek(&self) -> bool {
        self.worker.is_deepseek(&self.model)
    }

    /// Name shown for the worker: DeepSeek for a DeepSeek model, otherwise its family ("Codex", "Claude") or the source's own name.
    /// The task board adds the exact model after it.
    fn worker_label(&self) -> String {
        match self.worker.kind.as_str() {
            _ if self.worker_is_deepseek() => Actor::Worker.name().to_owned(),
            "codex_cli" => "Codex".to_owned(),
            "claude_cli" => "Claude".to_owned(),
            _ => self.worker.name.clone(),
        }
    }

    /// The worker is a CLI source (for example Codex): it edits the project in its own thread, kept between messages. Worker 2 is the
    /// same worker in a second thread; it runs beside the first, so it shows only its log lines (its text would mix into the first's turn).
    async fn work_cli(&self, task: &Delegation, attempt: usize, feedback: Option<&str>, editor: Actor) -> WorkerResult {
        let second = editor == Actor::Worker2;
        let label = if second { Actor::Worker2.name().to_owned() } else { self.worker_label() };
        let events = &self.events;
        if self.cancel.is_cancelled() || (!second && events.send(turn(&self.worker.kind, &self.model, &label)).await.is_err()) {
            self.cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        }
        let max = limit(self.can_escalate());
        let _ = events.send(ProviderEvent::text("log", format!("Giao {label}: {} (lần {attempt}/{max})", task.title.trim()))).await;
        let (session, tokens, worker_id) = if second {
            (&self.worker2_cli_session, &self.worker2_cli_tokens, worker2_key(&self.worker.id))
        } else {
            (&self.worker_cli_session, &self.worker_cli_tokens, self.worker.id.clone())
        };
        let mcp = self.mcp_for(&self.worker.kind);
        let request = ProviderRequest {
            provider: self.worker.clone(), model: self.model.clone(), folder: self.folder.clone(), permission: Permission::Write, effort: self.effort,
            messages: vec![message("user", self.with_codegraph_note(worker_prompt(&self.mutis, task, &preload(&self.folder, &task.files), feedback) + CLI_WORK_NOTE, &mcp))],
            session_id: session.lock().unwrap().clone(), mutis: self.mutis.clone(), mcp,
            // shortcut: a CLI cannot be scoped to files; the checker judges the diff instead.
            write_scope: Vec::new(), checker_session_id: None,
        };
        let size = (self.worker.kind == "codex_cli").then_some(tokens);
        let result = self.run_cli(&self.worker.kind, request, &label, session, size, second).await;
        // Kept in the chat record, so the next message resumes this thread instead of re-reading the project.
        let id = session.lock().unwrap().clone();
        if let Some(id) = id {
            let tokens = tokens.load(std::sync::atomic::Ordering::Relaxed);
            let _ = events.send(ProviderEvent { name: "worker_session", data: serde_json::json!({ "worker_id": worker_id, "session_id": id, "tokens": (tokens > 0).then_some(tokens) }) }).await;
        }
        result
    }

    /// Runs one CLI turn and folds its events into a `WorkerResult`. `session` receives the thread id the CLI reports; `label` prefixes its log lines.
    /// A `quiet` run forwards only its log lines to the chat.
    async fn run_cli(&self, kind: &str, request: ProviderRequest, label: &str, session: &Mutex<Option<String>>, size: Option<&std::sync::atomic::AtomicU64>, quiet: bool) -> WorkerResult {
        let events = &self.events;
        let mut result = WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        let inner = match for_kind(kind) {
            Ok(inner) => inner,
            Err(error) => {
                result.error = Some(error.to_string());
                return result;
            }
        };
        let (sender, mut receiver) = mpsc::channel(32);
        let cancel = self.cancel.clone();
        let handle = tokio::spawn(async move { inner.run(request, sender, cancel).await });
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => {
                    result.summary.push_str(event.data["text"].as_str().unwrap_or(""));
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                "reasoning" => {
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                "diff" => {
                    // A CLI repeats the whole diff of its turn: the last one is the result.
                    result.diff = event.data["text"].as_str().unwrap_or("").to_owned();
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                // Never forwarded: a "session" event would overwrite the coordinator's own thread in the chat record.
                "session" => {
                    if let Some(id) = event.data["session_id"].as_str() {
                        *session.lock().unwrap() = Some(id.to_owned());
                    }
                }
                "log" => {
                    let line = event.data["text"].as_str().unwrap_or("");
                    if let (Some(size), Some(tokens)) = (size, input_tokens(line)) {
                        size.store(tokens, std::sync::atomic::Ordering::Relaxed);
                    }
                    let line = if is_token_line(line) { line.to_owned() } else { format!("{label}: {line}") };
                    let _ = events.send(ProviderEvent::text("log", line)).await;
                }
                "error" => {
                    result.error = event.data["text"].as_str().map(str::to_owned);
                    if !quiet {
                        let _ = events.send(event).await;
                    }
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
            if !quiet && result.error.as_deref() != Some(error.as_str()) {
                let _ = events.send(ProviderEvent::text("error", error.clone())).await;
            }
            result.error = Some(error);
        }
        result
    }
}

#[async_trait::async_trait]
impl Backend for Live {
    async fn work(&self, task: &Delegation, attempt: usize, feedback: Option<&str>, editor: Actor) -> WorkerResult {
        if self.worker_is_cli() {
            return self.work_cli(task, attempt, feedback, editor).await;
        }
        // Worker 2 is the same API worker in a second conversation, and shows only its log lines (see `work_cli`).
        let quiet = editor == Actor::Worker2;
        let events = &self.events;
        let label = if quiet { Actor::Worker2.name().to_owned() } else { self.worker_label() };
        if self.cancel.is_cancelled() || (!quiet && events.send(turn(if self.worker_is_deepseek() { "deepseek" } else { &self.worker.kind }, &self.model, &label)).await.is_err()) {
            self.cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        }
        let max = limit(self.can_escalate());
        let _ = events.send(ProviderEvent::text("log", format!("Giao {label}: {} (lần {attempt}/{max})", task.title.trim()))).await;
        // The worker stays at Write whatever the chat permission is: it edits files but never runs commands (the checker runs the tests).
        let mut provider = self.worker.clone();
        provider.kind = "deepseek_agent".into();
        provider.agent_capable = true;
        let worker_request = ProviderRequest {
            provider, model: self.model.clone(), folder: self.folder.clone(), permission: Permission::Write, effort: Effort::Default,
            messages: vec![message("user", worker_prompt(&self.mutis, task, &preload(&self.folder, &task.files), feedback))], session_id: Some(if quiet { worker2_key(&self.worker_session) } else { self.worker_session.clone() }), mutis: self.mutis.clone(), mcp: Vec::new(),
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
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                "reasoning" => {
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                "diff" => {
                    result.diff.push_str(event.data["text"].as_str().unwrap_or(""));
                    if !quiet {
                        let _ = events.send(event).await;
                    }
                }
                "log" => {
                    let line = event.data["text"].as_str().unwrap_or("");
                    // The agent logs "Từ chối: <path> (ngoài phạm vi được giao)" when the scope stops an edit.
                    if line.contains("ngoài phạm vi") {
                        result.refused.push(line.trim_start_matches("Từ chối: ").to_owned());
                    }
                    // The token table and task_end only match lines that start with "Token:", so those stay unprefixed.
                    let line = worker_log(line, &label);
                    let _ = events.send(ProviderEvent::text("log", line)).await;
                }
                "error" => {
                    result.error = event.data["text"].as_str().map(str::to_owned);
                    // A worker failure stays visible in its own turn; the coordinator still gets it in the report.
                    if !quiet {
                        let _ = events.send(event).await;
                    }
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
            if !quiet && result.error.as_deref() != Some(error.as_str()) {
                let _ = events.send(ProviderEvent::text("error", error.clone())).await;
            }
            result.error = Some(error);
        }
        result
    }

    fn has_checker(&self) -> bool {
        self.checker.is_some()
    }

    async fn check(&self, task: &Delegation, attempt: usize, work: &WorkerResult) -> CheckResult {
        let Some((provider, model, effort)) = self.checker.clone() else { return CheckResult::unclear("no checker is configured") };
        let id = check_id(task, attempt);
        // The live card appears before the checker provider starts, so Tester is visible while it runs.
        let _ = self.events.send(checker_activity(&id, "start", task, attempt, &model, "")).await;
        let _turn = self.tester_busy.lock().await;
        let session = self.checker_session.lock().unwrap().clone();
        let folder = std::path::Path::new(&self.folder);
        let custom = crate::verify::command_for(&self.mutis, folder);
        let commands = if custom.trim().is_empty() { crate::verify::detected_commands(folder).into_iter().map(|(_, command)| command).collect() } else { vec![custom] };
        let mcp = self.mcp_for("codex_cli");
        let request = ProviderRequest {
            // The checker has the permission the user chose for the chat (Full lets it run the tests); the prompt forbids editing.
            provider, model: model.clone(), folder: self.folder.clone(), permission: self.permission, effort,
            messages: vec![message("user", self.with_codegraph_note(mutis_checker::prompt(&task.title, &task.files, &task.acceptance, &work.summary, &work.diff, &commands), &mcp))],
            session_id: session, mutis: self.mutis.clone(), mcp, write_scope: Vec::new(), checker_session_id: None,
        };
        let inner = match for_kind("codex_cli") {
            Ok(inner) => inner,
            Err(error) => {
                let _ = self.events.send(checker_activity(&id, "error", task, attempt, &model, &error.to_string())).await;
                return CheckResult::unclear(&error.to_string());
            }
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
                    let line = event.data["text"].as_str().unwrap_or("");
                    // Keep only the token line so Tester's usage reaches the token table; it is forwarded once by `announce_check`.
                    if is_token_line(line) {
                        *self.checker_tokens.lock().unwrap() = Some(line.to_owned());
                    } else if let Some(progress) = checker_progress(line) {
                        // Meaningful tool activity only: the live card shows it, the chat message never receives it.
                        let _ = self.events.send(checker_activity(&id, "progress", task, attempt, &model, &progress)).await;
                    }
                }
                "error" => {
                    error = event.data["text"].as_str().map(str::to_owned);
                    if let Some(why) = error.as_deref() {
                        let _ = self.events.send(checker_activity(&id, "error", task, attempt, &model, why)).await;
                    }
                }
                _ => {}
            }
        }
        match handle.await {
            Ok(Err(failure)) => error = Some(failure.to_string()),
            Err(failure) => error = Some(failure.to_string()),
            Ok(Ok(())) => {}
        }
        if let Some(why) = error.as_deref() {
            // A cancelled or failed check keeps its card, marked as failed, until the verdict turn replaces it.
            let _ = self.events.send(checker_activity(&id, "error", task, attempt, &model, why)).await;
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
        let who = Actor::Tester;
        if self.events.send(turn("codex_cli", model, who.name())).await.is_err() {
            self.cancel.cancel();
            return;
        }
        // Tester's thread is kept in the chat record.
        let session = self.checker_session.lock().unwrap().clone();
        if let Some(id) = session {
            let _ = self.events.send(ProviderEvent { name: "checker_session", data: serde_json::json!({ "session_id": id }) }).await;
        }
        let _ = self.events.send(ProviderEvent::text("delta", format!("**{} kiểm tra**\n\n{}\n", who.name(), check_text(task, attempt, check)))).await;
        let tokens = self.checker_tokens.lock().unwrap().take();
        if let Some(line) = tokens {
            let _ = self.events.send(ProviderEvent::text("log", line)).await;
        }
        // Only now, once the verdict turn, its session and its token line are out, the live card is removed:
        // a finished check waiting for the scheduler keeps showing until the chat has the real verdict.
        let _ = self.events.send(checker_activity(&check_id(task, attempt), "done", task, attempt, model, "")).await;
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn can_edit(&self) -> bool {
        self.can_edit
    }

    fn splits_big_small(&self) -> bool {
        self.worker_is_deepseek()
    }

    fn has_second_worker(&self) -> bool {
        true
    }

    fn actor_name(&self, actor: Actor) -> String {
        if actor == Actor::Worker { self.worker_label() } else { actor.name().to_owned() }
    }

    fn actor_model(&self, actor: Actor) -> Option<String> {
        match actor {
            Actor::Worker | Actor::Worker2 => Some(self.model.clone()),
            Actor::Tester => self.checker.as_ref().map(|(_, model, _)| model.clone()),
        }
    }

    async fn board(&self, rows: &[Row]) {
        let tasks: Vec<_> = rows.iter().map(|row| serde_json::json!({
            "title": row.title, "state": row.state, "attempt": row.attempt, "check": row.check,
            "actor": row.actor, "model": row.model, "checker": row.checker, "checker_model": row.checker_model,
        })).collect();
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
            worker: self.worker.clone(), model: self.model.clone(), effort: self.effort, worker_cli_session: Mutex::new(self.worker_session.clone()), worker_cli_tokens: Default::default(),
            worker2_cli_session: Mutex::new(self.worker2_session.clone()), worker2_cli_tokens: Default::default(),
            folder: request.folder.clone(), mutis: request.mutis.clone(),
            events: events.clone(), cancel: cancel.clone(), checker: self.checker.clone(),
            worker_session: format!("{}{}:{}:{}", super::worker_session::PREFIX, self.chat_id, self.worker.id, self.model),
            checker_session: Mutex::new(request.checker_session_id.clone()),
            tester_busy: Default::default(),
            checker_tokens: Mutex::new(None),
            can_edit: request.permission != Permission::Read,
            permission: request.permission,
            codegraph: request.mcp.iter().find(|server| server.name == "codegraph").cloned(),
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
            let (tasks, ui_notes) = guard_ui(tasks, request.mutis.deepseek_ui || !live.worker_is_deepseek());
            if cancel.is_cancelled() || (tasks.is_empty() && errors.is_empty() && ui_notes.is_empty()) {
                return Ok(());
            }
            let mut reply = errors.iter().map(|e| format!("Problem: {e}. Resend it as valid JSON.\n")).collect::<String>();
            for note in &ui_notes {
                reply.push_str(&format!("{note}\n\n"));
            }
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
        assert_eq!(worker_log("Token: vào 10 (cache 2), ra 3 (suy luận 0)", "DeepSeek"), "Token: vào 10 (cache 2), ra 3 (suy luận 0)");
        assert_eq!(worker_log("Sửa a.rs", "DeepSeek"), "DeepSeek: Sửa a.rs");
        assert_eq!(worker_log("Sửa a.rs", "Tester"), "Tester: Sửa a.rs");
        assert!(is_token_line("Token: vào 1, ra 2") && !is_token_line("DeepSeek: Token: vào 1, ra 2"));
        assert_eq!(input_tokens("Token: vào 147202 (cache 145408), ra 2478"), Some(147202));
        assert_eq!(input_tokens("Codex: Command: x"), None);
    }

    #[test]
    fn ui_tasks_never_go_to_deepseek_while_the_switch_is_off() {
        let task = |title: &str, files: &[&str], deps: &[&str]| Delegation {
            title: title.into(), files: files.iter().map(|f| f.to_string()).collect(), instructions: "i".into(), acceptance: vec![],
            depends_on: deps.iter().map(|d| d.to_string()).collect(), hard: false, trivial: false,
        };
        assert!(is_ui_file("web/src/a.CSS") && is_ui_file("x.scss") && is_ui_file("page.html") && !is_ui_file("web/src/a.tsx") && !is_ui_file("web/src/a.ts") && !is_ui_file("src/a.rs") && !is_ui_file("Makefile"));
        let tasks = || vec![task("logic", &["src/a.rs"], &[]), task("layout", &["web/a.css", "src/b.rs"], &[]), task("after", &["src/c.rs"], &["Layout"])];
        // Switch on: untouched.
        let (kept, notes) = guard_ui(tasks(), true);
        assert_eq!((kept.len(), notes.len()), (3, 0));
        // Off: the UI task and what depends on it go back to the coordinator.
        let (kept, notes) = guard_ui(tasks(), false);
        assert_eq!(kept.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["logic"]);
        assert_eq!(notes.len(), 2);
        // A task the coordinator already marked hard is left alone: it does it itself.
        let hard = Delegation { hard: true, ..task("layout", &["a.css"], &[]) };
        assert_eq!(guard_ui(vec![hard], false).0.len(), 1);
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
        let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
        let idle = WorkerResult { summary: "I only read files".into(), diff: String::new(), error: None, refused: Vec::new() };
        let text = report(&task, "DeepSeek", 2, 4, 1, &idle);
        assert!(text.contains("lần 2/4") && text.contains("KHÔNG") && text.contains("còn 2 lần"));
        let edited = WorkerResult { summary: "done".into(), diff: "+ new line".into(), error: Some("boom".into()), refused: Vec::new() };
        let text = report(&task, "DeepSeek", 4, 4, 0, &edited);
        assert!(text.contains("LỖI: boom") && text.contains("+ new line") && text.contains("còn 0 lần"));
    }

    #[test]
    fn report_headings_are_vietnamese_and_keep_the_evidence() {
        let task = Delegation { title: "Sửa đăng nhập".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
        let edited = WorkerResult { summary: "đã sửa".into(), diff: "+ let a = 1;".into(), error: None, refused: Vec::new() };
        let text = report(&task, "Worker 2", 1, 3, 2, &edited);
        // App-generated headings are Vietnamese.
        assert!(text.contains("Báo cáo của worker Worker 2"), "{text}");
        assert!(text.contains("Trạng thái: đã xong") && text.contains("Các file đã đổi: có, diff:"));
        assert!(text.contains("Tóm tắt của worker:") && text.contains("Ứng dụng đã tự chạy lại worker 2 lần"));
        // Identifiers, the attempt counter and the evidence stay verbatim.
        assert!(text.contains("\"Sửa đăng nhập\"") && text.contains("lần 1/3") && text.contains("còn 2 lần"));
        assert!(text.contains("+ let a = 1;") && text.contains("đã sửa"));
        // No English heading survives.
        for old in ["worker report", "Status:", "Files changed:", "Worker summary:", "attempts left", "NONE -"] {
            assert!(!text.contains(old), "old English label {old} in {text}");
        }
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
    fn only_cli_roles_get_the_codegraph_tool_and_only_when_the_lead_has_it() {
        let server = McpServerConfig { name: "codegraph".into(), command: "codegraph".into(), args: vec![], env: Default::default(), enabled: true };
        for kind in ["codex_cli", "claude_cli"] {
            assert_eq!(cli_mcp(Some(&server), kind).len(), 1);
            assert!(cli_mcp(None, kind).is_empty(), "no index or option off: no tool");
        }
        assert!(cli_mcp(Some(&server), "openai_compat").is_empty(), "an API worker cannot call MCP");
    }

    #[test]
    fn worker_prompt_carries_the_task_files_and_criteria() {
        let task = Delegation { title: "T".into(), files: vec!["a.rs".into(), "b.rs".into()], instructions: "change x".into(), acceptance: vec!["builds".into()], depends_on: vec![], hard: false, trivial: false };
        let prompt = worker_prompt(&MutisConfig::default(), &task, "FILE: a.rs\nfn a() {}\n```\n", None);
        assert!(prompt.contains("Task: T") && prompt.contains("Files: a.rs, b.rs") && prompt.contains("- builds"));
        assert!(prompt.contains("FILE: a.rs") && !worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("already read"));
        assert!(worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("previous attempt failed: it changed no file"));
    }

    #[test]
    fn worker_prompt_requires_vietnamese_prose_even_with_english_instructions() {
        let task = Delegation {
            title: "T".into(),
            files: vec!["a.rs".into()],
            instructions: "change x".into(),
            acceptance: vec![],
            depends_on: vec![],
            hard: false,
            trivial: false,
        };
        let prompt = worker_prompt(&MutisConfig::default(), &task, "", None);
        // Applied at runtime, so a saved configuration without the rule still receives it.
        assert!(prompt.contains(crate::core::rule::mutis_high_delegation::VIETNAMESE_RULE));
        assert!(prompt.contains("user-facing word in Vietnamese") && prompt.contains("even when the task instructions are in English"));
        // The English task text and the internal instructions are untouched.
        assert!(prompt.contains("Instructions:\nchange x") && prompt.contains("Make the edits yourself with the file tools"));
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

    #[test]
    fn a_check_keeps_one_identity_from_start_to_verdict() {
        let task = Delegation { title: " Fix Login ".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
        // The same check keeps its id in `check` and `announce_check`; a retry and another task never collide.
        assert_eq!(check_id(&task, 1), "fix login#1");
        assert_eq!(check_id(&task, 1), check_id(&task, 1));
        assert_ne!(check_id(&task, 1), check_id(&task, 2));
        let other = Delegation { title: "Fix Login #1".into(), ..task.clone() };
        assert_ne!(check_id(&task, 1), check_id(&other, 1));
    }

    #[test]
    fn checker_activity_carries_tester_identity_and_never_a_chat_turn() {
        let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
        let event = checker_activity("a#1", "start", &task, 1, "gpt-5-codex", "");
        assert_eq!(event.name, "checker_activity");
        assert!(event.data.get("id").is_none());
        assert_eq!(event.data["checker_activity"]["id"], "a#1");
        assert_eq!(event.data["checker_activity"]["phase"], "start");
        assert_eq!(event.data["checker_activity"]["title"], "A");
        assert_eq!(event.data["checker_activity"]["attempt"], 1);
        assert_eq!(event.data["checker_activity"]["model"], "gpt-5-codex");
        assert_eq!(event.data["checker_activity"]["provider_name"], "Người kiểm tra");
        assert_eq!(event.data["checker_activity"]["text"], "");
        assert_eq!(checker_activity("a#1", "done", &task, 1, "gpt-5-codex", "").data["checker_activity"]["phase"], "done");
    }

    #[test]
    fn checker_progress_keeps_tool_lines_and_drops_the_raw_verdict() {
        assert_eq!(checker_progress("  chạy cargo test  ").as_deref(), Some("chạy cargo test"));
        assert_eq!(checker_progress("{\"verdict\":\"PASS\"}"), None);
        assert_eq!(checker_progress("[tool] read"), None);
        assert_eq!(checker_progress("```json"), None);
        assert_eq!(checker_progress("   "), None);
        // 200 characters plus the "…" that `truncate_chars` appends when it cuts.
        assert_eq!(checker_progress(&"x".repeat(400)).map(|line| line.chars().count()), Some(201));
    }
}
