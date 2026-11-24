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
    /// CLI providers run commands themselves; API sources get run_command.
    Full,
}

/// File tools an API-key source may use in agent mode.
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
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

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
    /// MutiS High: the independent checker (Codex) keeps its own thread, resumed for every check instead of started anew.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checker_session_id: Option<String>,
    /// MutiS delegation: the CLI worker's own thread per worker source id, so the next message resumes what it just did.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub worker_sessions: std::collections::BTreeMap<String, String>,
    /// The effort the user picked at the MutiS source (max/xhigh = three AIs, else High). None = a single model, not MutiS.
    /// `provider_id` above is the lead MutiS resolved to, so this is what tells the two modes apart when the chat is reopened.
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

    /// Append this chat's current cumulative token totals to its own history file, if they changed.
    pub fn record_token_usage(
        &self,
        chat: &ChatRecord,
    ) -> Result<Option<crate::features::token_usage::TokenSnapshot>> {
        crate::features::token_usage::record(&self.directory, chat)
    }

    /// The latest token snapshots of one chat; a chat without history returns an empty list.
    pub fn token_history(
        &self,
        id: &str,
    ) -> Result<Vec<crate::features::token_usage::TokenSnapshot>> {
        crate::features::token_usage::history(&self.directory, id)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        crate::providers::worker_session::remove_chat(&self.directory, id);
        let path = self.file_path(id);
        if !path.exists() {
            return Ok(false);
        }
        fs::remove_file(&path)
            .with_context(|| format!("Could not delete chat {}", path.display()))?;
        Ok(true)
    }
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
        updated_at: unix_time(),
        plan_task_of: request.task_id.as_ref().and(request.plan_id.clone()),
    }
}

pub fn parse_effort(value: &str) -> Option<Effort> {
    match value {
        "default" => Some(Effort::Default),
        "none" => Some(Effort::None),
        "minimal" => Some(Effort::Minimal),
        "low" => Some(Effort::Low),
        "medium" => Some(Effort::Medium),
        "high" => Some(Effort::High),
        "xhigh" => Some(Effort::Xhigh),
        "max" => Some(Effort::Max),
        "ultra" => Some(Effort::Ultra),
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

/// The file tools of a chat at the MutiS source, whose lead is only known on the server: API-key agent sources get them
/// (read-only when the chat permission is read), CLI sources edit files themselves.
pub fn file_tools_for(provider: &crate::config::ProviderConfig, permission: Permission) -> FileTools {
    let agent = provider.kind == "anthropic" || (provider.kind == "openai_compat" && provider.agent_capable);
    match (agent, permission) {
        (false, _) => FileTools::None,
        (true, Permission::Read) => FileTools::Read,
        (true, _) => FileTools::Write,
    }
}

/// Decide how a chat message is run for a source:
/// - `Ok(None)`: normal chat (no file tools);
/// - `Ok(Some(permission))`: read-only (`Read`), file-write (`Write`), or command-enabled (`Full`) agent mode;
/// - `Err(message)`: the user asked for file tools on a source that does not allow them.
pub fn agent_permission(
    provider: &crate::config::ProviderConfig,
    file_tools: FileTools,
    permission: Permission,
) -> Result<Option<Permission>, &'static str> {
    if file_tools == FileTools::None {
        return Ok(None);
    }
    if provider.kind != "anthropic" && (provider.kind != "openai_compat" || !provider.agent_capable)
    {
        return Err("Bật «Cho phép AI đọc/sửa file» cho nguồn này trong Cài đặt");
    }
    Ok(Some(match file_tools {
        FileTools::Read => Permission::Read,
        FileTools::Write if permission == Permission::Full => Permission::Full,
        FileTools::Write => Permission::Write,
        FileTools::None => unreachable!(),
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
    fn mutis_file_tools_follow_the_leads_kind_and_the_permission() {
        assert_eq!(file_tools_for(&source("codex_cli", false), Permission::Write), FileTools::None);
        assert_eq!(file_tools_for(&source("openai_compat", false), Permission::Write), FileTools::None);
        assert_eq!(file_tools_for(&source("openai_compat", true), Permission::Read), FileTools::Read);
        assert_eq!(file_tools_for(&source("anthropic", false), Permission::Full), FileTools::Write);
    }

    #[test]
    fn no_file_tools_is_a_normal_chat_for_any_source() {
        assert_eq!(
            agent_permission(
                &source("anthropic", false),
                FileTools::None,
                Permission::Write
            ),
            Ok(None)
        );
        assert_eq!(
            agent_permission(
                &source("openai_compat", true),
                FileTools::None,
                Permission::Write
            ),
            Ok(None)
        );
    }

    #[test]
    fn file_tools_map_to_agent_read_or_write() {
        let deploy = source("openai_compat", true);
        assert_eq!(
            agent_permission(&deploy, FileTools::Read, Permission::Write),
            Ok(Some(Permission::Read))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Write, Permission::Write),
            Ok(Some(Permission::Write))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Write, Permission::Full),
            Ok(Some(Permission::Full))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Read, Permission::Full),
            Ok(Some(Permission::Read))
        );
    }

    #[test]
    fn anthropic_api_is_agent_capable_and_receives_full_permission() {
        let anthropic = source("anthropic", false);
        assert_eq!(
            agent_permission(&anthropic, FileTools::Write, Permission::Full),
            Ok(Some(Permission::Full))
        );
        assert_eq!(
            agent_permission(&anthropic, FileTools::Read, Permission::Write),
            Ok(Some(Permission::Read))
        );
    }

    #[test]
    fn file_tools_are_refused_when_the_source_is_not_agent_capable() {
        assert!(agent_permission(
            &source("openai_compat", false),
            FileTools::Write,
            Permission::Write
        )
        .is_err());
        assert!(agent_permission(
            &source("codex_cli", true),
            FileTools::Read,
            Permission::Write
        )
        .is_err());
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

#[cfg(test)]
mod message_model_tests {
    use super::*;

    #[test]
    fn old_message_json_without_model_loads_with_an_empty_model() {
        let message: ChatMessage =
            serde_json::from_str(r#"{"role":"assistant","content":"old reply"}"#).unwrap();
        assert_eq!(message.model, "");
        assert_eq!(message.content, "old reply");
    }

    #[test]
    fn old_chat_record_json_without_model_loads_with_empty_models() {
        let record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": "c", "title": "t", "provider_id": "p", "model": "m", "folder": "f",
            "updated_at": 0,
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ]
        }))
        .unwrap();
        assert_eq!(record.messages[0].model, "");
        assert_eq!(record.messages[1].model, "");
    }

    #[test]
    fn assistant_placeholder_requires_absent_tasks() {
        let mut message = ChatMessage::assistant("codex_cli", "test-model".into());
        assert_eq!(message.provider, "codex_cli");
        assert_eq!(message.model, "test-model");
        assert!(message.is_empty_placeholder());

        message.tasks = Some(Vec::new());
        assert!(!message.is_empty_placeholder());
    }

    #[test]
    fn empty_model_is_not_written_but_a_real_model_roundtrips() {
        let unknown = ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "assistant".into(),
            content: "hi".into(),
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: String::new(),
            model: String::new(),
            error: None,
        };
        let json = serde_json::to_value(&unknown).unwrap();
        assert!(json.get("model").is_none(), "unknown model stays out of the JSON");
        let known = ChatMessage {
            model: "claude-sonnet-4".into(),
            ..unknown
        };
        let json = serde_json::to_value(&known).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4");
        let back: ChatMessage = serde_json::from_value(json).unwrap();
        assert_eq!(back.model, "claude-sonnet-4");
    }
}

#[cfg(test)]
mod switch_source_tests {
    use super::*;

    fn record() -> ChatRecord {
        serde_json::from_value(serde_json::json!({
            "id": "c", "title": "t", "provider_id": "claude-cli", "model": "m", "folder": "f",
            "messages": [], "session_id": "claude-session", "updated_at": 0
        }))
        .unwrap()
    }

    #[test]
    fn switching_away_and_back_resumes_each_sources_own_session() {
        let mut chat = record();
        chat.switch_source("codex-cli");
        assert_eq!(chat.session_id, None, "the new source has no session yet");
        chat.provider_id = "codex-cli".into();
        chat.session_id = Some("codex-session".into());
        chat.switch_source("claude-cli");
        assert_eq!(chat.session_id.as_deref(), Some("claude-session"));
        chat.provider_id = "claude-cli".into();
        chat.switch_source("codex-cli");
        assert_eq!(chat.session_id.as_deref(), Some("codex-session"));
    }

    #[test]
    fn the_same_source_as_mutis_lead_and_on_its_own_keep_separate_threads() {
        let mut chat = record();
        chat.provider_id = "codex-cli".into();
        chat.mutis_level = Some("medium".into());
        chat.session_id = Some("lead-thread".into());
        // The user leaves MutiS for the same Codex: another thread, not the lead's.
        chat.switch_source(&thread_key("codex-cli", false));
        assert_eq!(chat.session_id, None);
        chat.mutis_level = None;
        chat.session_id = Some("own-thread".into());
        chat.switch_source(&thread_key("codex-cli", true));
        assert_eq!(chat.session_id.as_deref(), Some("lead-thread"));
        chat.mutis_level = Some("medium".into());
        chat.switch_source(&thread_key("codex-cli", false));
        assert_eq!(chat.session_id.as_deref(), Some("own-thread"));
    }

    #[test]
    fn a_thread_over_the_budget_is_dropped_and_zero_turns_it_off() {
        let mut chat = record();
        chat.worker_sessions.insert("w1".into(), "w".into());
        chat.thread_tokens.insert("main".into(), 160_000);
        chat.thread_tokens.insert("worker:w1".into(), 90_000);
        assert!(chat.rotate_big_threads(0).is_empty(), "0 = off");
        assert_eq!(chat.session_id.as_deref(), Some("claude-session"));
        assert_eq!(chat.rotate_big_threads(150_000), vec![("main".to_owned(), 160_000)]);
        assert_eq!(chat.session_id, None);
        assert!(chat.worker_sessions.contains_key("w1"), "under the budget: kept");
        assert_eq!(chat.rotate_big_threads(50_000), vec![("worker:w1".to_owned(), 90_000)]);
        assert!(chat.worker_sessions.is_empty());
        assert!(chat.thread_tokens.is_empty());
    }
}
