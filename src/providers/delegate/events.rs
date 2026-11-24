use super::{Delegation, ProviderEvent};
use crate::chat::ChatMessage;
use crate::core::text::truncate_chars;

/// A new chronological turn: the browser and the saved record start a fresh assistant message for this provider.
pub(super) fn turn(provider_kind: &str, model: &str, name: &str) -> ProviderEvent {
    ProviderEvent {
        name: "turn",
        data: serde_json::json!({ "provider_kind": provider_kind, "model": model, "provider_name": name }),
    }
}

pub(super) fn is_token_line(line: &str) -> bool {
    line.starts_with("Token:")
}

/// The input tokens of a `Token: vào N (cache C), ra M` line.
pub(super) fn input_tokens(line: &str) -> Option<u64> {
    let digits: String = line.strip_prefix("Token: vào ")?.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// A worker log line as shown in the chat; token lines stay unprefixed because the token table and task_end only match them at the start.
pub(super) fn worker_log(line: &str, label: &str) -> String {
    if is_token_line(line) { line.to_owned() } else { format!("{label}: {line}") }
}

pub(super) fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        debate_id: None, plan_id: None, role: role.into(), content, attachments: Vec::new(), reasoning: String::new(),
        logs: Vec::new(), diff: String::new(), files: Vec::new(), tasks: None, provider: String::new(), model: String::new(), error: None,
    }
}

/// Stable identity of one check: the task title plus its attempt number. Attempts are counted per task across
/// rounds, so the same check keeps one id in `check` and `announce_check` and no two checks collide.
pub(super) fn check_id(task: &Delegation, attempt: usize) -> String {
    format!("{}#{}", task.title.trim().to_lowercase(), attempt)
}

/// A live checker card event: shown beside the conversation while Tester runs, never appended to a chat message.
pub(super) fn checker_activity(id: &str, phase: &str, task: &Delegation, attempt: usize, model: &str, text: &str) -> ProviderEvent {
    ProviderEvent {
        name: "checker_activity",
        data: serde_json::json!({ "checker_activity": {
            "id": id, "phase": phase, "title": task.title.trim(), "attempt": attempt,
            "model": model, "provider_name": "Người kiểm tra", "text": text,
        } }),
    }
}

/// A checker log line worth showing on the live card: tool activity and commands, never the raw verdict JSON.
pub(super) fn checker_progress(line: &str) -> Option<String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('{') || line.starts_with('[') || line.starts_with("```") {
        return None;
    }
    Some(truncate_chars(line, 200))
}
