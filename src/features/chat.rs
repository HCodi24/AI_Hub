use anyhow::{Context, Result};
use axum::response::sse::Event;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Read,
    #[default]
    Write,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[default]
    Default,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatAttachment {
    pub id: String,
    pub mime: String,
    #[serde(default)]
    pub path: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<ChatAttachment>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reasoning: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub diff: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

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
    pub effort: Effort,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatSummary {
    pub id: String,
    pub title: String,
    pub updated_at: u64,
    #[serde(default)]
    pub dev_chat: bool,
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
    pub effort: Option<String>,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub task_kind: Option<String>,
}

#[derive(Clone)]
pub struct ChatStore {
    directory: PathBuf,
}

impl ChatStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    fn file_path(&self, id: &str) -> PathBuf {
        self.directory.join(format!("{id}.json"))
    }

    pub fn list(&self) -> Result<Vec<ChatSummary>> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }
        let mut chats = Vec::new();
        for entry in fs::read_dir(&self.directory)
            .with_context(|| format!("Could not list chats in {}", self.directory.display()))?
        {
            let entry = entry?;
            if entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("json")
            {
                continue;
            }
            let content = match fs::read_to_string(entry.path()) {
                Ok(content) => content,
                Err(_) => continue,
            };
            if let Ok(chat) = serde_json::from_str::<ChatRecord>(&content) {
                chats.push(ChatSummary {
                    id: chat.id,
                    title: chat.title,
                    updated_at: chat.updated_at,
                    dev_chat: chat.dev_chat,
                });
            }
        }
        chats.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(chats)
    }

    pub fn get(&self, id: &str) -> Result<Option<ChatRecord>> {
        let path = self.file_path(id);
        if !path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Could not read chat {}", path.display()))?;
        let chat = serde_json::from_str(&content)
            .with_context(|| format!("Could not parse chat {}", path.display()))?;
        Ok(Some(chat))
    }

    pub fn save(&self, chat: &ChatRecord) -> Result<()> {
        fs::create_dir_all(&self.directory).with_context(|| {
            format!(
                "Could not create chat directory {}",
                self.directory.display()
            )
        })?;
        let path = self.file_path(&chat.id);
        let json = serde_json::to_string_pretty(chat).context("Could not encode chat")?;
        fs::write(&path, json).with_context(|| format!("Could not save chat {}", path.display()))
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        let path = self.file_path(id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .with_context(|| format!("Could not delete chat {}", path.display()))?;
        Ok(true)
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
        effort: request
            .effort
            .as_deref()
            .and_then(parse_effort)
            .unwrap_or_default(),
        messages: request.messages.clone(),
        session_id: None,
        updated_at: unix_time(),
    }
}

pub fn parse_effort(value: &str) -> Option<Effort> {
    match value {
        "default" => Some(Effort::Default),
        "low" => Some(Effort::Low),
        "medium" => Some(Effort::Medium),
        "high" => Some(Effort::High),
        "xhigh" => Some(Effort::Xhigh),
        "max" => Some(Effort::Max),
        _ => None,
    }
}

pub fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

pub fn sse_event(name: &'static str, data: &Value) -> Event {
    let encoded = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_owned());
    Event::default().event(name).data(encoded)
}

pub fn save_record(store: &ChatStore, chat: &mut ChatRecord) -> Result<()> {
    chat.updated_at = unix_time();
    store.save(chat)
}

pub fn config_chat_dir(config_path: &Path) -> Result<PathBuf> {
    crate::core::paths::chats_dir(config_path)
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

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
