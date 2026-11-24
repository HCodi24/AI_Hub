use serde_json::Value;
use std::path::PathBuf;

pub(super) const KEEP_HISTORY_CHARS: usize = 240_000;

/// Where the chats (and the worker sessions inside them) live; None only when the config location is unknown.
pub(super) fn chats_root() -> Option<PathBuf> {
    crate::config::config_file_path().ok().and_then(|path| crate::chat::config_chat_dir(&path).ok())
}

/// A stop can land between an assistant's tool calls and their results; the API rejects a history that ends like that.
pub(super) fn drop_unanswered_calls(history: &mut Vec<Value>) {
    let Some(at) = history.iter().rposition(|m| m["tool_calls"].as_array().is_some_and(|calls| !calls.is_empty())) else { return };
    let calls = history[at]["tool_calls"].as_array().map_or(0, Vec::len);
    let answers = history[at + 1..].iter().take_while(|m| m["role"] == "tool").count();
    if answers < calls {
        history.truncate(at);
    }
}

/// The newest part of a conversation that fits the budget, starting at a user message (a tool result needs the call before it).
pub(super) fn trim_history(history: &[Value]) -> Vec<Value> {
    let (mut total, mut start) = (0, history.len());
    for (index, message) in history.iter().enumerate().rev() {
        total += message.to_string().len();
        if total > KEEP_HISTORY_CHARS {
            break;
        }
        start = index;
    }
    let from = history[start..].iter().position(|m| m["role"] == "user").map_or(history.len(), |at| start + at);
    history[from..].to_vec()
}
