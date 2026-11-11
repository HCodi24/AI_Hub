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
    /// Edit files and run commands in the working directory (CLI providers only).
    Full,
}

/// File tools an OpenAI-compatible source may use in agent mode (never commands).
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileTools {
    #[default]
    None,
    Read,
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
    pub file_tools: FileTools,
    #[serde(default)]
    pub effort: Effort,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub session_id: Option<String>,
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
                if chat.plan_task_of.is_some() {
                    continue;
                }
                chats.push(ChatSummary {
                    id: chat.id,
                    title: chat.title,
                    updated_at: chat.updated_at,
                    dev_chat: chat.dev_chat,
                    folder: chat.folder,
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
        file_tools: request.file_tools,
        effort: request
            .effort
            .as_deref()
            .and_then(parse_effort)
            .unwrap_or_default(),
        messages: request.messages.clone(),
        session_id: None,
        updated_at: unix_time(),
        plan_task_of: request.task_id.as_ref().and(request.plan_id.clone()),
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

/// Decide how a chat message is run for a source:
/// - `Ok(None)`: normal chat (no file tools);
/// - `Ok(Some(permission))`: agent mode with read-only (`Read`) or full (`Write`) file tools;
/// - `Err(message)`: the user asked for file tools on a source that does not allow them.
pub fn agent_permission(
    provider: &crate::config::ProviderConfig,
    file_tools: FileTools,
) -> Result<Option<Permission>, &'static str> {
    if file_tools == FileTools::None {
        return Ok(None);
    }
    if provider.kind != "openai_compat" || !provider.agent_capable {
        return Err("Bật «Cho phép AI đọc/sửa file» cho nguồn này trong Cài đặt");
    }
    Ok(Some(match file_tools {
        FileTools::Read => Permission::Read,
        _ => Permission::Write,
    }))
}

#[cfg(test)]
mod file_tools_tests {
    use super::*;
    use crate::config::ProviderConfig;

    #[test]
    fn full_permission_roundtrips_and_write_stays_default() {
        let full: Permission = serde_json::from_str("\"full\"").unwrap();
        assert_eq!(full, Permission::Full);
        assert_eq!(serde_json::to_string(&full).unwrap(), "\"full\"");
        assert_eq!(Permission::default(), Permission::Write);
    }

    fn source(kind: &str, agent_capable: bool) -> ProviderConfig {
        ProviderConfig {
            id: "p".into(),
            kind: kind.into(),
            agent_capable,
            ..Default::default()
        }
    }

    #[test]
    fn no_file_tools_is_a_normal_chat_for_any_source() {
        assert_eq!(
            agent_permission(&source("anthropic", false), FileTools::None),
            Ok(None)
        );
        assert_eq!(
            agent_permission(&source("openai_compat", true), FileTools::None),
            Ok(None)
        );
    }

    #[test]
    fn file_tools_map_to_agent_read_or_write() {
        let deploy = source("openai_compat", true);
        assert_eq!(
            agent_permission(&deploy, FileTools::Read),
            Ok(Some(Permission::Read))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Write),
            Ok(Some(Permission::Write))
        );
    }

    #[test]
    fn file_tools_are_refused_when_the_source_is_not_agent_capable() {
        assert!(agent_permission(&source("openai_compat", false), FileTools::Write).is_err());
        assert!(agent_permission(&source("codex_cli", true), FileTools::Read).is_err());
    }

    #[test]
    fn file_tools_values_deserialize_from_lowercase_and_default_to_none() {
        let tools: FileTools = serde_json::from_str("\"read\"").unwrap();
        assert_eq!(tools, FileTools::Read);
        assert_eq!(FileTools::default(), FileTools::None);
    }
}

#[cfg(test)]
mod plan_task_chat_tests {
    use super::*;

    #[test]
    fn plan_task_chats_are_hidden_from_the_history_list() {
        let directory = std::env::temp_dir().join(format!("ai-hub-chat-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let request = |plan: Option<&str>, task: Option<&str>| -> ChatRequest {
            serde_json::from_value(serde_json::json!({
                "chat_id": uuid::Uuid::new_v4().to_string(),
                "provider_id": "p",
                "model": "m",
                "folder": "C:/x",
                "messages": [{"role": "user", "content": "hi"}],
                "plan_id": plan,
                "task_id": task,
            }))
            .unwrap()
        };
        let normal = new_record(&request(None, None));
        let task = new_record(&request(Some("plan-1"), Some("t1")));
        let plan_only = new_record(&request(Some("plan-1"), None));
        assert_eq!(task.plan_task_of.as_deref(), Some("plan-1"));
        assert!(plan_only.plan_task_of.is_none());
        for chat in [&normal, &task, &plan_only] {
            store.save(chat).unwrap();
        }
        let ids: Vec<String> = store.list().unwrap().into_iter().map(|c| c.id).collect();
        assert!(ids.contains(&normal.id) && ids.contains(&plan_only.id));
        assert!(!ids.contains(&task.id));
        let _ = std::fs::remove_dir_all(directory);
    }
}
