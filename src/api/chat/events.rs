//! Helpers for the event loop of one chat run: idle watchdog, empty-reply check, persisting streamed events.
use crate::{
    chat::{self, ChatRecord, ChatStore},
    providers::ProviderEvent,
};
use std::time::{Duration, Instant};

pub(super) fn idle_timeout_reached(idle: Duration, minutes: u32) -> bool {
    minutes > 0 && idle >= Duration::from_secs(u64::from(minutes) * 60)
}

pub(super) fn should_emit_empty_reply(has_delta: bool, has_error: bool, cancelled: bool) -> bool {
    !has_delta && !has_error && !cancelled
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
            let _ = chat::save_record(store, record);
            *last_persist = Instant::now();
        }
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
        let mut clean = record.clone();
        if let Some(message) = clean.messages.last_mut() {
            message.content = crate::features::memory::strip_blocks(&message.content).0;
        }
        let _ = chat::save_record(store, &mut clean);
        *last_persist = Instant::now();
    }
}

#[cfg(test)]
mod tests {
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
}
