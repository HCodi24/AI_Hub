//! MutiS High chat: Codex coordinates, the app runs the DeepSeek worker and hands the real report back to Codex.
//!
//! Codex ends a turn with fenced `mutis-delegate` JSON blocks (see `core::rule::mutis_high_delegation`). The app runs
//! each block on the worker, then resumes the same Codex thread with the worker's report. The app, not Codex, counts
//! the attempts per task and tells Codex to take a task over once the limit is spent.
use super::schedule::run_round;
use super::{for_kind, ProviderEvent, ProviderRequest};
use crate::chat::{Effort, Permission};
use crate::config::ProviderConfig;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod events;
mod live;
mod parse;
mod report;

use events::{message, turn};
use live::Live;
pub use parse::{check_files, extract};
use parse::guard_ui;
pub(super) use report::{auto_retry_reason, report, WorkerResult};
use report::REVIEW_INSTRUCTION;

pub const MAX_ATTEMPTS: usize = 4;
/// Empty or failed worker runs the app repeats by itself (they still count as attempts) before the coordinator hears about them.
pub(super) const MAX_AUTO_RETRIES: usize = 2;
/// Guard against a Codex that keeps delegating: the number of Codex turns in one chat reply.
const MAX_ROUNDS: usize = 12;

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

pub struct DelegateProvider {
    /// The source that runs the tasks: an API agent (OpenAI-compatible) or a CLI (Codex, Claude), whichever is active.
    pub worker: ProviderConfig,
    pub model: String,
    /// Reasoning effort of the worker role; used when the worker is a CLI.
    pub effort: Effort,
    /// The independent checker (any source, its model and effort), when the option is on and configured.
    pub checker: Option<(ProviderConfig, String, Effort)>,
    /// Shown once in the chat when the option is on but the checker cannot run (what to set in the High section).
    pub checker_note: Option<String>,
    pub chat_id: String,
    /// The lead (coordinator) source kind: `codex_cli` or `claude_cli`.
    pub lead_kind: String,
    /// The CLI worker's thread kept in the chat record from an earlier message; None = start a new one.
    pub worker_session: Option<String>,
    /// The same for Worker 2, the second slot of the same worker.
    pub worker2_session: Option<String>,
}

/// Key under which Worker 2's thread is kept in the chat record, next to the worker's own (`worker_id`).
pub fn worker2_key(worker_id: &str) -> String {
    format!("{worker_id}#2")
}

impl DelegateProvider {
    /// One Codex turn: every event goes to the chat; returns the reply text and the native session id.
    async fn codex_turn(&self, request: &ProviderRequest, events: &mpsc::Sender<ProviderEvent>, cancel: &CancellationToken) -> Result<(String, Option<String>)> {
        // The coordinator gets its own turn before the call, so its text never mixes with a worker turn.
        if cancel.is_cancelled() { return Ok((String::new(), None)); }
        let lead_name = if self.lead_kind == "claude_cli" { "Claude" } else { "Codex" };
        if events.send(turn(&self.lead_kind, &request.model, lead_name)).await.is_err() {
            cancel.cancel();
            return Ok((String::new(), None));
        }
        let (sender, mut receiver) = mpsc::channel(32);
        let inner = for_kind(&self.lead_kind)?;
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
            // Only a Codex checker resumes the stored thread; another source (or a switch to one) starts afresh, each check being self-contained.
            checker_session: Mutex::new(request.checker_session_id.clone().filter(|_| self.checker.as_ref().is_some_and(|(p, _, _)| p.kind == "codex_cli"))),
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
mod tests;
