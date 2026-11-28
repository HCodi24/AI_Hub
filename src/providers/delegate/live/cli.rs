use super::{Live, CLI_WORK_NOTE};
use crate::{
    providers::{
        delegate::{
            events::{input_tokens, is_token_line, message, turn_with_continuation},
            report::{preload, worker_prompt, WorkerResult},
            snapshot::Snapshot,
            slot_key, Delegation,
        },
        for_kind,
        schedule::{limit, Actor, Backend},
        ProviderEvent, ProviderRequest,
    },
};
use std::sync::Mutex;
use tokio::sync::mpsc;

impl Live {
    /// Each CLI slot keeps its own thread between messages. Only the primary slot streams text to the chat.
    pub(in crate::providers::delegate) async fn work_cli(&self, task: &Delegation, attempt: usize, feedback: Option<&str>, slot: u8) -> WorkerResult {
        let quiet = slot != 0;
        let label = self.actor_name(Actor::Worker(slot));
        let events = &self.events;
        let same_message = !quiet && attempt > 1
            && self.last_worker_turn.lock().unwrap().as_deref() == Some(task.title.as_str());
        if self.cancel.is_cancelled()
            || (!quiet && events.send(turn_with_continuation(&self.worker.kind, &self.model, &label, same_message)).await.is_err())
        {
            self.cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        }
        if !quiet {
            *self.last_worker_turn.lock().unwrap() = Some(task.title.clone());
        }
        let max = limit(self.can_escalate());
        let _ = events.send(ProviderEvent::text("log", format!("Giao {label}: {} (lần {attempt}/{max})", task.title.trim()))).await;
        let session = &self.worker_cli_sessions[usize::from(slot)];
        let tokens = &self.worker_cli_tokens[usize::from(slot)];
        let worker_id = slot_key(&self.worker.id, slot);
        let mcp = self.mcp_for(&self.worker.kind);
        let request = ProviderRequest {
            provider: self.worker.clone(), model: self.model.clone(), folder: self.folder.clone(), permission: self.worker_permission, effort: self.effort,
            messages: vec![message("user", self.with_codegraph_note(worker_prompt(&self.mutis, task, &preload(&self.folder, &task.files), feedback) + CLI_WORK_NOTE, &mcp))],
            session_id: session.lock().unwrap().clone(), mutis: self.mutis.clone(), mcp,
            // shortcut: a CLI cannot be scoped to files; the checker judges the diff instead.
            write_scope: Vec::new(), checker_session_id: None,
        };
        let size = (self.worker.kind == "codex_cli").then_some(tokens);
        let folder = self.folder.clone();
        let files = task.files.clone();
        let snapshot = tokio::task::spawn_blocking(move || Snapshot::take(&folder, &files)).await;
        let mut result = self.run_cli(&self.worker.kind, request, &label, session, size, quiet).await;
        if result.diff.trim().is_empty() {
            if let Ok(snapshot) = snapshot {
                let folder = self.folder.clone();
                if let Ok(diff) = tokio::task::spawn_blocking(move || snapshot.diff(&folder)).await {
                    if !diff.is_empty() {
                        let _ = events.send(ProviderEvent::text(
                            "log",
                            format!("{label}: diff lấy từ so sánh file (CLI không báo diff)"),
                        )).await;
                        if !quiet {
                            let _ = events.send(ProviderEvent::text("diff", diff.clone())).await;
                        }
                    }
                    result.diff = diff;
                }
            }
        }
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
