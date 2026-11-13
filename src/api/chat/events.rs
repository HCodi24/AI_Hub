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
    for message in clean.messages.iter_mut().filter(|message| message.role == "assistant") {
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
    if event.name == "session" {
        if let Some(session_id) = event.data["session_id"]
            .as_str()
            .or_else(|| event.data["text"].as_str())
        {
            record.session_id = Some(session_id.to_owned());
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
        if record.messages.last().is_some_and(is_empty_placeholder) {
            if let Some(message) = record.messages.last_mut() {
                message.provider = kind.to_owned();
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
                provider: kind.to_owned(),
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
            persist_event(&store, &mut record, &ProviderEvent { name, data }, &mut last);
        };
        send("turn", serde_json::json!({"provider_kind":"codex_cli"}));
        send("delta", serde_json::json!({"text":"Delegate.<memory>none</memory>"}));
        send("session", serde_json::json!({"session_id":"session"}));
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
        assert_eq!(saved.messages.iter().skip(1).map(|m| m.provider.as_str()).collect::<Vec<_>>(),
            ["codex_cli", "deepseek", "codex_cli"]);
        assert_eq!(saved.messages[1].content, "Delegate.");
        assert!(record.messages[1].content.contains("<memory>"));
        assert_eq!(saved.session_id.as_deref(), Some("session"));
        assert_eq!(saved.messages[2].content, "Worker result");
        assert_eq!(saved.messages[2].diff, "+change");
        assert_eq!(saved.messages[2].error.as_deref(), Some("Worker failed"));
        assert!(saved.messages[3].error.is_none());
        assert_eq!(saved.messages[3].content, "Review");
        std::fs::remove_file(directory.join(format!("{id}.json"))).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }
}
