use super::{ChatRecord, ChatSummary};
use anyhow::{Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

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

pub fn valid_id(id: &str) -> bool {
    uuid::Uuid::parse_str(id).is_ok()
}

pub fn save_record(store: &ChatStore, chat: &mut ChatRecord) -> Result<()> {
    chat.updated_at = crate::core::time::now_secs();
    store.save(chat)
}

pub fn config_chat_dir(config_path: &Path) -> Result<PathBuf> {
    crate::core::paths::chats_dir(config_path)
}
