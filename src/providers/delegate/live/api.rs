use super::Live;
use crate::chat::{Effort, Permission};
use crate::providers::{
    delegate::{
        events::{message, turn, worker_log},
        report::{preload, worker_prompt},
        worker2_key, Delegation, WorkerResult,
    },
    for_kind,
    schedule::{limit, Actor, Backend},
    ProviderEvent, ProviderRequest,
};
use tokio::sync::mpsc;

impl Live {
    pub(super) async fn work_api(
        &self,
        task: &Delegation,
        attempt: usize,
        feedback: Option<&str>,
        editor: Actor,
    ) -> WorkerResult {
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
}
