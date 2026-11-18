//! Helpers for the event loop of one chat run: idle watchdog, empty-reply check, persisting streamed events.
use crate::{
    chat::{self, ChatMessage, ChatRecord, ChatStore},
    providers::ProviderEvent,
};
use std::time::{Duration, Instant};

pub(super) fn idle_timeout_reached(idle: Duration, minutes: u32) -> bool {
    minutes > 0 && idle >= Duration::from_secs(u64::from(minutes) * 60)
}

pub(super) fn should_emit_empty_reply(has_delta: bool, has_error: bool, cancelled: bool) -> bool {
    !has_delta && !has_error && !cancelled
}

/// Provider kinds allowed to open a turn: the Codex coordinator and the DeepSeek worker.
fn turn_provider(kind: &str) -> Option<&str> {
    matches!(kind, "codex_cli" | "deepseek").then_some(kind)
}

/// A trailing assistant message with nothing in it yet: a new turn may reuse it instead of adding a blank one.
fn is_empty_placeholder(message: &ChatMessage) -> bool {
    message.role == "assistant"
        && message.content.is_empty()
        && message.reasoning.is_empty()
        && message.logs.is_empty()
        && message.diff.is_empty()
        && message.files.is_empty()
        && message.attachments.is_empty()
        && message.error.is_none()
        && message.plan_id.is_none()
        && message.debate_id.is_none()
}

/// Save a copy with memory protocol blocks stripped from every assistant message; the in-memory text stays raw.
fn persist_clean(store: &ChatStore, record: &mut ChatRecord) {
    let mut clean = record.clone();
    for message in clean
        .messages
        .iter_mut()
        .filter(|message| message.role == "assistant")
    {
        message.content = crate::features::memory::strip_blocks(&message.content).0;
    }
    let _ = chat::save_record(store, &mut clean);
}

pub(super) fn persist_event(
    store: &ChatStore,
    record: &mut ChatRecord,
    event: &ProviderEvent,
    last_persist: &mut Instant,
) {
    if event.name == "worker_session" {
        if let (Some(worker), Some(id)) = (
            event.data["worker_id"].as_str(),
            event.data["session_id"].as_str(),
        ) {
            record
                .worker_sessions
                .insert(worker.to_owned(), id.to_owned());
            if let Some(tokens) = event.data["tokens"].as_u64() {
                record
                    .thread_tokens
                    .insert(format!("worker:{worker}"), tokens);
            }
            persist_clean(store, record);
            *last_persist = Instant::now();
        }
        return;
    }
    // Size of the main thread after its last model call; saved with the next write of the record.
    if event.name == "thread_tokens" {
        if let Some(tokens) = event.data["text"].as_str().and_then(|t| t.parse().ok()) {
            record.thread_tokens.insert("main".to_owned(), tokens);
        }
        return;
    }
    if matches!(event.name, "session" | "checker_session") {
        if let Some(session_id) = event.data["session_id"]
            .as_str()
            .or_else(|| event.data["text"].as_str())
        {
            // The checker's thread is kept apart from the coordinator's so each resumes its own conversation.
            if event.name == "checker_session" {
                record.checker_session_id = Some(session_id.to_owned());
            } else {
                record.session_id = Some(session_id.to_owned());
            }
            persist_clean(store, record);
            *last_persist = Instant::now();
        }
        return;
    }
    // A turn boundary comes before the message lookup: it decides which assistant message the next events land in.
    if event.name == "turn" {
        let Some(kind) = event.data["provider_kind"].as_str().and_then(turn_provider) else {
            return;
        };
        // The turn's own model, never the previous turn's: a missing model stays unknown (empty).
        let model = event.data["model"].as_str().unwrap_or_default().to_owned();
        if record.messages.last().is_some_and(is_empty_placeholder) {
            if let Some(message) = record.messages.last_mut() {
                message.provider = kind.to_owned();
                message.model = model;
            }
        } else {
            record.messages.push(ChatMessage {
                debate_id: None,
                plan_id: None,
                role: "assistant".to_owned(),
                content: String::new(),
                reasoning: String::new(),
                logs: Vec::new(),
                diff: String::new(),
                files: Vec::new(),
                tasks: None,
                provider: kind.to_owned(),
                model,
                error: None,
                attachments: Vec::new(),
            });
        }
        persist_clean(store, record);
        *last_persist = Instant::now();
        return;
    }
    let Some(message) = record
        .messages
        .last_mut()
        .filter(|message| message.role == "assistant")
    else {
        return;
    };
    let text = event.data["text"].as_str().unwrap_or_default();
    let force_persist = match event.name {
        "delta" => {
            message.content.push_str(text);
            false
        }
        "reasoning" => {
            message.reasoning.push_str(text);
            false
        }
        "log" => {
            message.logs.push(text.to_owned());
            // A valid token report is the only log that produces a token observation; the
            // cumulative totals are recorded right away, independent of the event-log settings.
            if crate::features::token_usage::is_token_report(text) {
                if let Err(error) = store.record_token_usage(record) {
                    eprintln!("Could not record token usage: {error:#}");
                }
            }
            true
        }
        "diff" => {
            message.diff = text.to_owned();
            true
        }
        "error" => {
            message.error = Some(text.to_owned());
            true
        }
        _ => return,
    };
    if force_persist || last_persist.elapsed() >= Duration::from_secs(1) {
        // Keep raw streamed text in RAM until final extraction; never persist protocol blocks.
        persist_clean(store, record);
        *last_persist = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::{idle_timeout_reached, should_emit_empty_reply};
    use std::time::Duration;
    #[test]
    fn idle_watchdog_obeys_off_and_timeout_threshold() {
        assert!(!idle_timeout_reached(Duration::from_secs(7200), 0));
        assert!(!idle_timeout_reached(Duration::from_secs(599), 10));
        assert!(idle_timeout_reached(Duration::from_secs(600), 10));
    }
    #[test]
    fn emits_empty_reply_only_when_unanswered_and_not_cancelled() {
        assert!(should_emit_empty_reply(false, false, false));
        assert!(!should_emit_empty_reply(true, false, false));
        assert!(!should_emit_empty_reply(false, true, false));
        assert!(!should_emit_empty_reply(false, false, true));
    }

    #[test]
    fn chronological_turns_persist_and_route_worker_errors_without_mixing_content() {
        let directory = std::env::temp_dir().join(format!("mutis-turns-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let mut record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": id, "title": "turn test", "provider_id": "codex", "model": "test", "folder": "",
            "updated_at": 0, "messages": [{"role":"user","content":"task"}, {"role":"assistant","content":""}]
        })).unwrap();
        let mut last = Instant::now();
        let mut send = |name: &'static str, data: serde_json::Value| {
            persist_event(
                &store,
                &mut record,
                &ProviderEvent { name, data },
                &mut last,
            );
        };
        send("turn", serde_json::json!({"provider_kind":"codex_cli"}));
        send(
            "delta",
            serde_json::json!({"text":"Delegate.<memory>none</memory>"}),
        );
        send("session", serde_json::json!({"session_id":"session"}));
        send(
            "checker_session",
            serde_json::json!({"session_id":"checker"}),
        );
        send(
            "worker_session",
            serde_json::json!({"worker_id":"codex-luna","session_id":"worker"}),
        );
        send("turn", serde_json::json!({"provider_kind":"deepseek"}));
        send("log", serde_json::json!({"text":"Read file"}));
        send("delta", serde_json::json!({"text":"Worker result"}));
        send("diff", serde_json::json!({"text":"+change"}));
        send("error", serde_json::json!({"text":"Worker failed"}));
        send("turn", serde_json::json!({"provider_kind":"codex_cli"}));
        send("delta", serde_json::json!({"text":"Review"}));
        send("log", serde_json::json!({"text":"Reviewed"}));
        send("turn", serde_json::json!({"provider_kind":"invalid"}));
        let saved = store.get(&id).unwrap().unwrap();
        assert_eq!(saved.messages.len(), 4);
        assert_eq!(
            saved
                .messages
                .iter()
                .skip(1)
                .map(|m| m.provider.as_str())
                .collect::<Vec<_>>(),
            ["codex_cli", "deepseek", "codex_cli"]
        );
        assert_eq!(saved.messages[1].content, "Delegate.");
        assert!(record.messages[1].content.contains("<memory>"));
        assert_eq!(saved.session_id.as_deref(), Some("session"));
        assert_eq!(
            saved.checker_session_id.as_deref(),
            Some("checker"),
            "the checker keeps its own thread id"
        );
        assert_eq!(
            saved.worker_sessions.get("codex-luna").map(String::as_str),
            Some("worker")
        );
        assert_eq!(saved.messages[2].content, "Worker result");
        assert_eq!(saved.messages[2].diff, "+change");
        assert_eq!(saved.messages[2].error.as_deref(), Some("Worker failed"));
        assert!(saved.messages[3].error.is_none());
        assert_eq!(saved.messages[3].content, "Review");
        assert!(
            saved
                .messages
                .iter()
                .all(|message| message.model.is_empty()),
            "turns without a model stay unknown instead of borrowing one"
        );
        std::fs::remove_file(directory.join(format!("{id}.json"))).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn each_turn_keeps_its_own_model_and_a_missing_model_stays_unknown() {
        let directory = std::env::temp_dir().join(format!("mutis-models-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let mut record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": id, "title": "model test", "provider_id": "codex", "model": "test", "folder": "",
            "updated_at": 0, "messages": [{"role":"user","content":"task"}]
        }))
        .unwrap();
        let mut last = Instant::now();
        let mut send = |name: &'static str, data: serde_json::Value| {
            persist_event(
                &store,
                &mut record,
                &ProviderEvent { name, data },
                &mut last,
            );
        };
        send(
            "turn",
            serde_json::json!({"provider_kind":"codex_cli","model":"gpt-5-codex"}),
        );
        send("delta", serde_json::json!({"text":"First"}));
        send(
            "turn",
            serde_json::json!({"provider_kind":"deepseek","model":"deepseek-chat"}),
        );
        send("delta", serde_json::json!({"text":"Second"}));
        // A turn event without a model must not inherit the previous turn's identity.
        send("turn", serde_json::json!({"provider_kind":"codex_cli"}));
        send("delta", serde_json::json!({"text":"Third"}));
        send("log", serde_json::json!({"text":"Completed"}));

        let saved = store.get(&id).unwrap().unwrap();
        assert_eq!(saved.messages.len(), 4);
        assert_eq!(
            saved
                .messages
                .iter()
                .skip(1)
                .map(|m| m.model.as_str())
                .collect::<Vec<_>>(),
            ["gpt-5-codex", "deepseek-chat", ""],
            "models persist per message and a missing turn model stays empty"
        );
        assert_eq!(saved.messages[1].content, "First");
        assert_eq!(saved.messages[2].content, "Second");
        assert_eq!(saved.messages[3].content, "Third");
        assert_eq!(saved.messages[1].provider, "codex_cli");
        assert_eq!(saved.messages[2].provider, "deepseek");
        std::fs::remove_file(directory.join(format!("{id}.json"))).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn reused_placeholder_takes_the_new_turn_model_and_never_the_old_one() {
        let directory = std::env::temp_dir().join(format!("mutis-reuse-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let mut record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": id, "title": "reuse test", "provider_id": "codex", "model": "test", "folder": "",
            "updated_at": 0,
            "messages": [
                {"role":"user","content":"task"},
                {"role":"assistant","content":"","model":"stale-model"}
            ]
        }))
        .unwrap();
        let mut last = Instant::now();
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "turn",
                data: serde_json::json!({"provider_kind":"codex_cli"}),
            },
            &mut last,
        );
        let saved = store.get(&id).unwrap().unwrap();
        assert_eq!(saved.messages.len(), 2, "the empty placeholder is reused");
        assert_eq!(
            saved.messages[1].model, "",
            "a missing turn model clears the stale one"
        );
        std::fs::remove_file(directory.join(format!("{id}.json"))).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    fn token_store() -> (std::path::PathBuf, ChatStore, String, ChatRecord, Instant) {
        let directory = std::env::temp_dir().join(format!("mutis-tokens-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let id = uuid::Uuid::new_v4().to_string();
        let record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": id, "title": "token event test", "provider_id": "codex", "model": "test",
            "folder": "", "updated_at": 0, "messages": [{"role":"user","content":"task"}]
        }))
        .unwrap();
        (directory, store, id, record, Instant::now())
    }

    #[test]
    fn a_valid_token_log_records_history_without_the_frontend() {
        let (directory, store, id, mut record, mut last) = token_store();
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "turn",
                data: serde_json::json!({"provider_kind":"codex_cli","model":"gpt-5-codex"}),
            },
            &mut last,
        );
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "log",
                data: serde_json::json!({"text":"Token: vào 1200 (cache 300), ra 45 (suy luận 10)"}),
            },
            &mut last,
        );
        let history = store.token_history(&id).unwrap();
        assert_eq!(
            history.len(),
            1,
            "the backend records the observation itself"
        );
        assert_eq!(history[0].models.len(), 1);
        assert_eq!(history[0].models[0].model, "gpt-5-codex");
        assert_eq!(
            (history[0].models[0].input, history[0].models[0].output),
            (1200, 45)
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn unrelated_logs_never_create_token_history() {
        let (directory, store, id, mut record, mut last) = token_store();
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "turn",
                data: serde_json::json!({"provider_kind":"codex_cli","model":"gpt-5-codex"}),
            },
            &mut last,
        );
        for text in [
            "Read file",
            "Token: vào1, ra2",
            "Token: vào 1, ra 2, chi phí $x",
            "Completed",
        ] {
            persist_event(
                &store,
                &mut record,
                &ProviderEvent {
                    name: "log",
                    data: serde_json::json!({ "text": text }),
                },
                &mut last,
            );
        }
        assert!(store.token_history(&id).unwrap().is_empty());
        assert!(
            !directory.join("token-usage").exists(),
            "no file for logs that are not reports"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn spaced_dollar_reports_record_and_the_last_report_replaces_the_earlier_one() {
        let (directory, store, id, mut record, mut last) = token_store();
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "turn",
                data: serde_json::json!({"provider_kind":"codex_cli","model":"gpt-5-codex"}),
            },
            &mut last,
        );
        // `\$?\s*` in the frontend regex: whitespace after the dollar sign is valid.
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "log",
                data: serde_json::json!({"text":"Token: vào 10, ra 5, chi phí $ 0.12"}),
            },
            &mut last,
        );
        let first = store.token_history(&id).unwrap();
        assert_eq!(
            first.len(),
            1,
            "a spaced-dollar report is a valid observation"
        );
        assert_eq!(
            (first[0].models[0].input, first[0].models[0].output),
            (10, 5)
        );
        // A later report of the same message replaces the totals instead of adding to them.
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "log",
                data: serde_json::json!({"text":"Token: vào 20, ra 6, chi phí $ 0.24"}),
            },
            &mut last,
        );
        let history = store.token_history(&id).unwrap();
        assert_eq!(history.len(), 2, "the earlier snapshot stays appended");
        assert_eq!(
            (history[0].models[0].input, history[0].models[0].output),
            (10, 5)
        );
        assert_eq!(
            (history[1].models[0].input, history[1].models[0].output),
            (20, 6),
            "the last valid report of the message wins"
        );
        assert!(history[1].at > history[0].at);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stored_totals_are_cumulative_and_never_added_snapshots() {
        let (directory, store, id, mut record, mut last) = token_store();
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "turn",
                data: serde_json::json!({"provider_kind":"codex_cli","model":"gpt-5-codex"}),
            },
            &mut last,
        );
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "log",
                data: serde_json::json!({"text":"Token: vào 10, ra 5"}),
            },
            &mut last,
        );
        persist_event(
            &store,
            &mut record,
            &ProviderEvent {
                name: "log",
                data: serde_json::json!({"text":"Token: vào 30, ra 7"}),
            },
            &mut last,
        );
        let history = store.token_history(&id).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(
            (history[0].models[0].input, history[0].models[0].output),
            (10, 5)
        );
        assert_eq!(
            (history[1].models[0].input, history[1].models[0].output),
            (30, 7),
            "the last report of the message replaces the earlier one instead of being added"
        );
        assert!(history[1].at > history[0].at);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
