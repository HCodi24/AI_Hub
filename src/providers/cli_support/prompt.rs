/// Which AI wrote a reply, as `ChatMessage::provider` labels it.
fn ai_label(kind: &str) -> &'static str {
    match kind {
        "codex_cli" => "Codex",
        "claude_cli" | "anthropic" => "Claude",
        "antigravity_cli" => "Antigravity",
        "deepseek" => "DeepSeek",
        _ => "AI",
    }
}

const TRANSCRIPT_MESSAGES: usize = 30;
const TRANSCRIPT_CHARS: usize = 40_000;
/// The answer right before the user's new message matters most, so it keeps (almost) everything.
const LAST_ANSWER_CHARS: usize = 12_000;
const OLDER_MESSAGE_CHARS: usize = 2_500;

/// Newest messages first until the budget is spent, printed oldest first. Done once, when a CLI has no session
/// for this chat (a source switch), so a larger budget costs little.
fn transcript(messages: &[crate::chat::ChatMessage]) -> String {
    let mut lines = Vec::new();
    let mut used = 0;
    for (age, message) in messages.iter().rev().filter(|m| !m.content.trim().is_empty()).enumerate() {
        if age >= TRANSCRIPT_MESSAGES || used >= TRANSCRIPT_CHARS {
            break;
        }
        let limit = if age == 0 && message.role == "assistant" { LAST_ANSWER_CHARS } else { OLDER_MESSAGE_CHARS };
        let who = match message.role.as_str() {
            "assistant" => format!("Assistant ({})", ai_label(&message.provider)),
            "system" => "System".to_owned(),
            _ => "User".to_owned(),
        };
        let cut = if message.error.as_deref() == Some(crate::chat::STOPPED_NOTE) { " [stopped before it finished]" } else { "" };
        let files = if message.files.is_empty() { String::new() } else {
            format!(" [files it changed: {}]", message.files.iter().take(12).map(|f| f.path.as_str()).collect::<Vec<_>>().join(", "))
        };
        let line = format!("{who}: {}{cut}{files}\n", crate::core::text::truncate_chars(message.content.trim(), limit));
        used += line.chars().count();
        lines.push(line);
    }
    lines.reverse();
    lines.concat()
}

/// Prompt for a CLI of `kind` (`codex_cli`, `claude_cli`):
/// - no session yet (first use, or the user switched to this source): a transcript of the conversation so far;
/// - session resumed but another AI answered since this CLI's last reply: only what it missed;
/// - otherwise just the latest message (the CLI remembers the rest).
pub fn compose_prompt_for(messages: &[crate::chat::ChatMessage], has_session: bool, kind: &str) -> String {
    let Some(latest) = messages.last() else {
        return String::new();
    };
    if messages.len() == 1 {
        return latest.content.clone();
    }
    let history = &messages[..messages.len() - 1];
    if has_session {
        // Old chats carry no author labels: assume the session knows everything.
        let Some(seen) = history.iter().rposition(|m| m.role == "assistant" && m.provider == kind) else {
            return latest.content.clone();
        };
        let missed = &history[seen + 1..];
        if !missed.iter().any(|m| m.role == "assistant" && !m.content.trim().is_empty()) {
            return latest.content.clone();
        }
        return format!(
            "While you were away, the conversation continued with other AIs:\n{}\nLatest user message:\n{}",
            transcript(missed),
            latest.content
        );
    }
    format!(
        "Earlier conversation for context (some answers come from other AIs):\n{}\nLatest user message:\n{}",
        transcript(history),
        latest.content
    )
}
