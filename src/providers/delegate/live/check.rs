use super::Live;
use crate::core::rule::mutis_checker;
use crate::providers::{
    delegate::{
        events::{check_id, checker_activity, checker_progress, is_token_line, message},
        Delegation, WorkerResult,
    },
    for_kind,
    schedule::{parse_check, CheckResult},
    ProviderRequest,
};
use tokio::sync::mpsc;

impl Live {
    pub(super) async fn run_check(
        &self,
        task: &Delegation,
        attempt: usize,
        work: &WorkerResult,
    ) -> CheckResult {
        let Some((provider, model, effort)) = self.checker.clone() else { return CheckResult::unclear("no checker is configured") };
        if let Some(why) = super::quota::frozen(&provider.id) {
            return CheckResult::unclear(&format!("the checker's usage limit is spent, so no check ran: {why}"));
        }
        let id = check_id(task, attempt);
        // The live card appears before the checker provider starts, so Tester is visible while it runs.
        let _ = self.events.send(checker_activity(&id, "start", task, attempt, &model, "")).await;
        let _turn = self.tester_busy.lock().await;
        let session = self.checker_session.lock().unwrap().clone();
        let folder = std::path::Path::new(&self.folder);
        let custom = crate::verify::command_for(&self.mutis, folder);
        let commands = if custom.trim().is_empty() { crate::verify::detected_commands(folder).into_iter().map(|(_, command)| command).collect() } else { vec![custom] };
        let kind = provider.kind.clone();
        let mcp = self.mcp_for(&kind);
        let request = ProviderRequest {
            // The checker uses its configured permission; the prompt forbids editing.
            provider, model: model.clone(), folder: self.folder.clone(), permission: self.checker_permission, effort,
            messages: vec![message("user", self.with_codegraph_note(mutis_checker::prompt(&task.title, &task.files, &task.acceptance, &work.summary, &work.diff, &commands, self.mutis.checker_app_rules, &self.mutis.checker_rules), &mcp))],
            session_id: session, mutis: self.mutis.clone(), mcp, write_scope: Vec::new(), checker_session_id: None,
        };
        let inner = match for_kind(&kind) {
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
                // The checker may have to ask before running a command: the question goes to the user, whose answer reaches it by id.
                "permission_request" | "permission_denied" => {
                    let _ = self.events.send(event).await;
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
}
