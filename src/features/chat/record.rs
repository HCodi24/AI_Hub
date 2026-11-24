use super::{parse_effort, ChatMessage, Effort, FileTools, Permission};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatRecord {
    pub id: String,
    pub title: String,
    pub provider_id: String,
    pub model: String,
    pub folder: String,
    #[serde(default)]
    pub dev_chat: bool,
    #[serde(default)]
    pub permission: Permission,
    #[serde(default)]
    pub file_tools: FileTools,
    #[serde(default)]
    pub effort: Effort,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub session_id: Option<String>,
    /// The independent MutiS checker keeps its own thread, resumed for each check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker_session_id: Option<String>,
    /// MutiS delegation: the CLI worker's own thread per worker source id, so the next message resumes what it just did.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub worker_sessions: std::collections::BTreeMap<String, String>,
    /// MutiS chat marker. New runs write "mutis"; any saved string marks a MutiS chat.
    /// `provider_id` stores the resolved lead, so reopening uses this marker to distinguish the two modes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutis_level: Option<String>,
    /// Input tokens of the last turn of each CLI thread of this chat (`main`, `worker:<source id>`), to rotate a thread that grew too big.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub thread_tokens: std::collections::BTreeMap<String, u64>,
    /// CLI sessions of the sources this chat left, so switching back resumes them instead of starting over.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub sessions: std::collections::BTreeMap<String, String>,
    pub updated_at: u64,
    /// Set for the hidden chat a plan task runs in: its output is shown in the plan card, not in the chat history list.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_task_of: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    #[serde(default)]
    pub dev_chat: bool,
    pub folder: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatRequest {
    pub chat_id: String,
    pub provider_id: String,
    pub model: String,
    pub folder: String,
    #[serde(default)]
    pub dev_chat: bool,
    #[serde(default)]
    pub permission: Permission,
    #[serde(default)]
    pub file_tools: FileTools,
    #[serde(default)]
    pub effort: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub task_kind: Option<String>,
}

/// Key of a CLI thread in `ChatRecord::sessions`: the source, and whether it ran as the lead of MutiS. The same Codex as a
/// lead and on its own are two threads: the lead's carries the delegation rules, which a single model must not inherit.
pub fn thread_key(provider_id: &str, mutis: bool) -> String {
    if mutis {
        format!("{provider_id}#mutis")
    } else {
        provider_id.to_owned()
    }
}

impl ChatRecord {
    /// The thread this chat's `session_id` belongs to.
    pub fn thread_key(&self) -> String {
        thread_key(&self.provider_id, self.mutis_level.is_some())
    }

    /// The user picked another source or mode: park this thread's CLI session and resume the earlier one of `new_key`, if any.
    pub fn switch_source(&mut self, new_key: &str) {
        if let Some(current) = self.session_id.take() {
            self.sessions.insert(self.thread_key(), current);
        }
        self.session_id = self.sessions.remove(new_key);
        self.thread_tokens.remove("main");
    }

    /// Drops every CLI thread whose last turn read `budget` input tokens or more (0 = never), so the next message starts it afresh
    /// (the main thread then gets the recent conversation again, see `compose_prompt_for`). Returns what was dropped: (key, tokens).
    pub fn rotate_big_threads(&mut self, budget: u64) -> Vec<(String, u64)> {
        let mut dropped = Vec::new();
        if budget == 0 {
            return dropped;
        }
        let big: Vec<(String, u64)> = self
            .thread_tokens
            .iter()
            .filter(|(_, tokens)| **tokens >= budget)
            .map(|(key, tokens)| (key.clone(), *tokens))
            .collect();
        for (key, tokens) in big {
            let had_thread = match key.strip_prefix("worker:") {
                Some(worker) => self.worker_sessions.remove(worker).is_some(),
                None => self.session_id.take().is_some(),
            };
            self.thread_tokens.remove(&key);
            if had_thread {
                dropped.push((key, tokens));
            }
        }
        dropped
    }
}

pub fn new_record(request: &ChatRequest) -> ChatRecord {
    let title = request
        .messages
        .iter()
        .find(|message| message.role == "user")
        .map(|message| {
            message
                .content
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_else(|| "Chat mới".to_owned());
    ChatRecord {
        id: request.chat_id.clone(),
        title: truncate(&title, 72),
        provider_id: request.provider_id.clone(),
        model: request.model.clone(),
        folder: request.folder.clone(),
        dev_chat: request.dev_chat,
        permission: request.permission,
        file_tools: request.file_tools,
        effort: request
            .effort
            .as_deref()
            .and_then(parse_effort)
            .unwrap_or_default(),
        messages: request.messages.clone(),
        session_id: None,
        checker_session_id: None,
        worker_sessions: Default::default(),
        mutis_level: None,
        thread_tokens: Default::default(),
        sessions: Default::default(),
        updated_at: crate::core::time::now_secs(),
        plan_task_of: request.task_id.as_ref().and(request.plan_id.clone()),
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let result: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{result}…")
    } else {
        result
    }
}
