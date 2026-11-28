//! MutiS chat: the lead coordinates, and the app runs the worker and returns its report.
//!
//! The lead requests tasks with fenced `mutis-delegate` JSON blocks. The app runs each task on a worker,
//! uses the checker when configured, counts attempts, and asks the lead to take over once the limit is spent.
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
mod snapshot;

use events::{message, turn};
use live::Live;
pub use parse::{check_files, extract};
pub(super) use report::{auto_retry_reason, report, WorkerResult};
use report::REVIEW_INSTRUCTION;

pub const MAX_ATTEMPTS: usize = 4;
/// Empty or failed worker runs the app repeats by itself (they still count as attempts) before the coordinator hears about them.
pub(super) const MAX_AUTO_RETRIES: usize = 2;
/// Maximum number of lead turns in one chat reply.
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
    /// The coordinator marks a tiny low-risk change (a few lines of styling or text): Tester does not check it, the coordinator reads the diff.
    #[serde(default)]
    pub trivial: bool,
}

pub struct DelegateProvider {
    /// The configured API agent or CLI that runs the tasks.
    pub worker: ProviderConfig,
    pub model: String,
    /// Reasoning effort of the worker role.
    pub effort: Effort,
    pub worker_permission: Permission,
    pub worker_count: usize,
    /// The independent checker (any source, its model and effort), when the option is on and configured.
    pub checker: Option<(ProviderConfig, String, Effort)>,
    pub checker_permission: Permission,
    /// Shown once in the chat when the configured checker cannot run.
    pub checker_note: Option<String>,
    pub chat_id: String,
    /// The effective source kind of the lead.
    pub lead_kind: String,
    /// Saved CLI thread per worker slot; None starts a new thread.
    pub worker_sessions: Vec<Option<String>>,
}

pub(crate) const WORKER_SLOTS: u8 = 3;

/// The first slot keeps the unsuffixed key used by existing chats.
pub fn slot_key(worker_id: &str, slot: u8) -> String {
    if slot == 0 {
        worker_id.to_owned()
    } else {
        format!("{worker_id}#{}", slot + 1)
    }
}

impl DelegateProvider {
    /// One lead turn: every event goes to the chat; returns the reply text and the native session id.
    async fn lead_turn(&self, request: &ProviderRequest, events: &mpsc::Sender<ProviderEvent>, cancel: &CancellationToken) -> Result<(String, Option<String>)> {
        // The coordinator gets its own turn before the call, so its text never mixes with a worker turn.
        if cancel.is_cancelled() { return Ok((String::new(), None)); }
        let lead_name = match self.lead_kind.as_str() {
            "codex_cli" => "Codex",
            "claude_cli" => "Claude",
            _ => &request.provider.name,
        };
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
            worker: self.worker.clone(), model: self.model.clone(), effort: self.effort,
            worker_permission: self.worker_permission,
            worker_count: self.worker_count,
            checker_permission: self.checker_permission,
            worker_cli_sessions: std::array::from_fn(|slot| {
                Mutex::new(self.worker_sessions.get(slot).cloned().flatten())
            }),
            worker_cli_tokens: Default::default(),
            folder: request.folder.clone(), mutis: request.mutis.clone(),
            events: events.clone(), cancel: cancel.clone(), checker: self.checker.clone(),
            worker_session: format!("{}{}:{}:{}", super::worker_session::PREFIX, self.chat_id, self.worker.id, self.model),
            // Only a Codex checker resumes the stored thread; another source (or a switch to one) starts afresh, each check being self-contained.
            checker_session: Mutex::new(request.checker_session_id.clone().filter(|_| self.checker.as_ref().is_some_and(|(p, _, _)| p.kind == "codex_cli"))),
            tester_busy: Default::default(),
            checker_tokens: Mutex::new(None),
            last_worker_turn: Mutex::new(None),
            can_edit: request.permission != Permission::Read,
            codegraph: request.mcp.iter().find(|server| server.name == "codegraph").cloned(),
        });
        let mut attempts: HashMap<String, usize> = HashMap::new();
        let mut note = self.checker_note.clone();
        for _ in 0..MAX_ROUNDS {
            let (text, session) = self.lead_turn(&request, &events, &cancel).await?;
            *live.last_worker_turn.lock().unwrap() = None;
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
        let _ = events.send(ProviderEvent::text("log", format!("Dừng giao việc: quá {MAX_ROUNDS} vòng của Lead trong một lượt"))).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
