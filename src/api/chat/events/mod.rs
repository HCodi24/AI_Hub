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

/// Strip memory protocol blocks from assistant messages on disk while preserving the in-memory record.
fn persist_clean(store: &ChatStore, record: &mut ChatRecord) {
    let has_memory_block =
        |message: &ChatMessage| message.role == "assistant" && message.content.contains("<memory>");
    let result = if record.messages.iter().any(has_memory_block) {
        let mut clean = record.clone();
        for message in clean
            .messages
            .iter_mut()
            .filter(|message| has_memory_block(message))
        {
            message.content = crate::features::memory::strip_blocks(&message.content).0;
        }
        chat::save_record(store, &mut clean)
    } else {
        let updated_at = record.updated_at;
        let result = chat::save_record(store, record);
        record.updated_at = updated_at;
        result
    };
    if let Err(error) = result {
        eprintln!("Could not save chat record: {error:#}");
    }
}

fn save_now(store: &ChatStore, record: &mut ChatRecord, last_persist: &mut Instant) {
    persist_clean(store, record);
    *last_persist = Instant::now();
}

const WORKER_SESSION: &str = "worker_session";
const THREAD_TOKENS: &str = "thread_tokens";
const SESSION: &str = "session";
const CHECKER_SESSION: &str = "checker_session";
const TURN: &str = "turn";
const DELTA: &str = "delta";
const REASONING: &str = "reasoning";
const LOG: &str = "log";
const DIFF: &str = "diff";
const ERROR: &str = "error";

pub(super) fn persist_event(
    store: &ChatStore,
    record: &mut ChatRecord,
    event: &ProviderEvent,
    last_persist: &mut Instant,
) {
    if event.name == WORKER_SESSION {
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
            save_now(store, record, last_persist);
        }
        return;
    }
    // Size of the main thread after its last model call; saved with the next write of the record.
    if event.name == THREAD_TOKENS {
        if let Some(tokens) = event.data["text"].as_str().and_then(|t| t.parse().ok()) {
            record.thread_tokens.insert("main".to_owned(), tokens);
        }
        return;
    }
    if matches!(event.name, self::SESSION | self::CHECKER_SESSION) {
        if let Some(session_id) = event.data["session_id"]
            .as_str()
            .or_else(|| event.data["text"].as_str())
        {
            // The checker's thread is kept apart from the coordinator's so each resumes its own conversation.
            if event.name == CHECKER_SESSION {
                record.checker_session_id = Some(session_id.to_owned());
            } else {
                record.session_id = Some(session_id.to_owned());
            }
            save_now(store, record, last_persist);
        }
        return;
    }
    // A turn boundary comes before the message lookup: it decides which assistant message the next events land in.
    if event.name == TURN {
        let Some(kind) = event.data["provider_kind"].as_str().and_then(turn_provider) else {
            return;
        };
        // The turn's own model, never the previous turn's: a missing model stays unknown (empty).
        let model = event.data["model"].as_str().unwrap_or_default().to_owned();
        let same_message = event.data["same_message"].as_bool() == Some(true);
        if let Some(message) = record.messages.last_mut().filter(|message| {
            same_message
                && message.role == "assistant"
                && message.provider == kind
                && message.model.trim() == model.trim()
                && !message.is_empty_placeholder()
        }) {
            if !message.content.is_empty() && !message.content.ends_with("\n\n") {
                message.content.push_str("\n\n");
            }
        } else if let Some(message) = record
            .messages
            .last_mut()
            .filter(|message| message.is_empty_placeholder())
        {
            message.provider = kind.to_owned();
            message.model = model;
        } else {
            record.messages.push(ChatMessage::assistant(kind, model));
        }
        save_now(store, record, last_persist);
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
        self::DELTA => {
            message.content.push_str(text);
            false
        }
        self::REASONING => {
            message.reasoning.push_str(text);
            false
        }
        self::LOG => {
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
        self::DIFF => {
            message.diff = text.to_owned();
            true
        }
        self::ERROR => {
            message.error = Some(text.to_owned());
            true
        }
        _ => return,
    };
    if force_persist || last_persist.elapsed() >= Duration::from_secs(1) {
        // Keep raw streamed text in RAM until final extraction; never persist protocol blocks.
        save_now(store, record, last_persist);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[test]
fn continued_turn_reuses_the_matching_assistant_message() {
    let directory = std::env::temp_dir().join(format!("mutis-retry-{}", uuid::Uuid::new_v4()));
    let store = ChatStore::new(directory.clone());
    let mut record: ChatRecord = serde_json::from_value(serde_json::json!({
        "id": uuid::Uuid::new_v4().to_string(), "title": "retry test",
        "provider_id": "codex", "model": "test", "folder": "", "updated_at": 0,
        "messages": [{"role":"assistant","provider":"codex_cli","model":"test","content":"First"}]
    }))
    .unwrap();
    let event = ProviderEvent {
        name: TURN,
        data: serde_json::json!({"provider_kind":"codex_cli","model":" test ","same_message":true}),
    };
    let mut last = Instant::now();
    persist_event(&store, &mut record, &event, &mut last);
    persist_event(&store, &mut record, &event, &mut last);
    let saved = store.get(&record.id).unwrap().unwrap();
    assert_eq!(saved.messages.len(), 1);
    assert_eq!(saved.messages[0].content, "First\n\n");
    std::fs::remove_file(directory.join(format!("{}.json", record.id))).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
