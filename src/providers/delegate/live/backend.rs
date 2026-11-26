use super::Live;
use crate::providers::{
    delegate::{
        check_files,
        events::{check_id, checker_activity, turn},
        Delegation, WorkerResult,
    },
    schedule::{check_text, Actor, Backend, CheckResult, Row},
    ProviderEvent,
};

#[async_trait::async_trait]
impl Backend for Live {
    async fn work(&self, task: &Delegation, attempt: usize, feedback: Option<&str>, editor: Actor) -> WorkerResult {
        let Actor::Worker(slot) = editor else { unreachable!("Only workers edit delegated tasks") };
        if self.worker_is_cli() {
            self.work_cli(task, attempt, feedback, slot).await
        } else {
            self.work_api(task, attempt, feedback, slot).await
        }
    }

    fn has_checker(&self) -> bool {
        self.checker.is_some()
    }

    async fn check(&self, task: &Delegation, attempt: usize, work: &WorkerResult) -> CheckResult {
        self.run_check(task, attempt, work).await
    }

    async fn say(&self, line: String) {
        let _ = self.events.send(ProviderEvent::text("log", line)).await;
    }

    async fn announce_check(&self, task: &Delegation, attempt: usize, check: &CheckResult) {
        let Some((provider, model, _)) = &self.checker else { return };
        let name = &provider.name;
        *self.last_worker_turn.lock().unwrap() = None;
        if self.events.send(turn(&provider.kind, model, name)).await.is_err() {
            self.cancel.cancel();
            return;
        }
        // Tester's thread is kept in the chat record.
        let session = self.checker_session.lock().unwrap().clone();
        if let Some(id) = session {
            let _ = self.events.send(ProviderEvent { name: "checker_session", data: serde_json::json!({ "session_id": id }) }).await;
        }
        let _ = self.events.send(ProviderEvent::text("delta", format!("**{} kiểm tra**\n\n{}\n", name, check_text(task, attempt, check)))).await;
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

    async fn limited(&self, actor: Actor) -> Option<String> {
        let provider = match actor {
            Actor::Tester => &self.checker.as_ref()?.0,
            _ => &self.worker,
        };
        crate::features::providers::limits::frozen_now(provider).await
    }

    fn note_limit(&self, actor: Actor, why: &str) {
        if let Some(source) = self.source_of(actor) {
            super::quota::freeze(source, why);
        }
    }

    fn can_edit(&self) -> bool {
        self.can_edit
    }

    fn worker_slots(&self) -> usize {
        self.worker_count.clamp(1, 3)
    }

    fn actor_name(&self, actor: Actor) -> String {
        match actor {
            Actor::Worker(0) => self.worker_label(),
            // The checker may be any source: show its own name.
            Actor::Tester => self.checker.as_ref().map_or_else(
                || "Người kiểm tra".to_owned(),
                |(provider, _, _)| provider.name.clone(),
            ),
            Actor::Worker(slot) => format!("Worker {}", usize::from(slot) + 1),
        }
    }

    fn actor_model(&self, actor: Actor) -> Option<String> {
        match actor {
            Actor::Worker(_) => Some(self.model.clone()),
            Actor::Tester => self.checker.as_ref().map(|(_, model, _)| model.clone()),
        }
    }

    async fn board(&self, rows: &[Row]) {
        let tasks: Vec<_> = rows.iter().map(|row| serde_json::json!({
            "title": row.title, "state": row.state, "attempt": row.attempt, "check": row.check,
            "actor": row.actor, "model": row.model, "checker": row.checker, "checker_model": row.checker_model,
            "reason": row.reason,
        })).collect();
        let _ = self.events.send(ProviderEvent { name: "tasks", data: serde_json::json!({ "tasks": tasks }) }).await;
    }

    fn path_problems(&self, task: &Delegation) -> Vec<String> {
        check_files(&self.folder, &task.files)
    }
}
