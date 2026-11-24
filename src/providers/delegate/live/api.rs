use super::Live;
use crate::providers::{
    delegate::{
        events::{message, turn, worker_log},
        report::{preload, worker_prompt},
        slot_key, Delegation, WorkerResult,
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
        slot: u8,
    ) -> WorkerResult {
        // Each API slot has its own conversation; only the primary slot streams text.
        let quiet = slot != 0;
        let events = &self.events;
        let label = self.actor_name(Actor::Worker(slot));
        if self.cancel.is_cancelled() || (!quiet && events.send(turn(&self.worker.kind, &self.model, &label)).await.is_err()) {
            self.cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None, refused: Vec::new() };
        }
        let max = limit(self.can_escalate());
        let _ = events.send(ProviderEvent::text("log", format!("Giao {label}: {} (lần {attempt}/{max})", task.title.trim()))).await;
        let mut provider = self.worker.clone();
        if provider.kind == "openai_compat" {
            provider.kind = "deepseek_agent".into();
        }
        provider.agent_capable = true;
        let run = for_kind(&provider.kind);
        let worker_request = ProviderRequest {
            provider, model: self.model.clone(), folder: self.folder.clone(), permission: self.worker_permission, effort: self.effort,
            messages: vec![message("user", worker_prompt(&self.mutis, task, &preload(&self.folder, &task.files), feedback))], session_id: Some(slot_key(&self.worker_session, slot)), mutis: self.mutis.clone(), mcp: Vec::new(),
            write_scope: task.files.clone(),
            checker_session_id: None,
        };
        let (sender, mut receiver) = mpsc::channel(32);
        let inner_cancel = self.cancel.clone();
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
