use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatAttachment {
    pub id: String,
    pub mime: String,
    #[serde(default)]
    pub path: String,
}

/// `error` saved on an assistant reply the user stopped (or whose connection dropped): a reload still shows it was cut short.
pub const STOPPED_NOTE: &str = "Đã dừng";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub debate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub diff: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<crate::features::changes::FileChange>,
    /// Optional MutiS task metadata archived with the assistant reply that completed the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tasks: Option<Vec<Value>>,
    /// Provider kind that wrote this assistant reply (e.g. `codex_cli`), for its avatar in the UI.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub provider: String,
    /// Model that wrote this assistant reply, kept per message so the Dashboard can show it. Empty = unknown (old chats).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ChatMessage {
    pub fn assistant(provider: &str, model: String) -> Self {
        Self {
            role: "assistant".into(),
            content: String::new(),
            debate_id: None,
            plan_id: None,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: provider.into(),
            model,
            error: None,
        }
    }

    pub fn is_empty_placeholder(&self) -> bool {
        self.role == "assistant"
            && self.content.is_empty()
            && self.debate_id.is_none()
            && self.plan_id.is_none()
            && self.attachments.is_empty()
            && self.reasoning.is_empty()
            && self.logs.is_empty()
            && self.diff.is_empty()
            && self.files.is_empty()
            && self.tasks.is_none()
            && self.error.is_none()
    }
}
