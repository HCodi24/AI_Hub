//! The DeepSeek worker's conversation, one file per chat and source: `<chats>/worker-sessions/<chat uuid>/<provider>_<model>.json`.
//!
//! The API keeps no thread, so the app keeps it (a CLI keeps its own and the chat record stores only its id). The key is
//! `worker:<chat uuid>:<provider id>:<model>`; the chat uuid picks the folder, the rest the file, so a session is never read
//! by another chat, source or model. Deleting the chat deletes the folder.
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A `session_id` with this prefix keeps the worker's conversation (reads, edits, answers) between runs of one chat.
pub const PREFIX: &str = "worker:";

fn dir(root: &Path) -> PathBuf {
    root.join("worker-sessions")
}

// shortcut: odd provider ids or models that differ only by symbols share a file; ids and models are slugs in practice.
fn file(root: &Path, key: &str) -> Option<PathBuf> {
    let (chat, rest) = key.strip_prefix(PREFIX)?.split_once(':')?;
    if !crate::chat::valid_id(chat) {
        return None;
    }
    let name: String = rest.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') { c } else { '_' }).collect();
    Some(dir(root).join(chat).join(format!("{name}.json")))
}

/// Missing, unreadable or corrupt files all mean "no earlier conversation".
pub fn load(root: &Path, key: &str) -> Vec<Value> {
    file(root, key)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save(root: &Path, key: &str, history: &[Value]) {
    let Some(path) = file(root, key) else { return };
    let Some(parent) = path.parent() else { return };
    let Ok(text) = serde_json::to_string(history) else { return };
    // Write beside the file and rename, so a crash never leaves half a conversation.
    let temp = path.with_extension("tmp");
    if std::fs::create_dir_all(parent).is_ok() && std::fs::write(&temp, text).is_ok() {
        let _ = std::fs::rename(&temp, &path);
    }
}

pub fn remove_chat(root: &Path, chat_id: &str) {
    if crate::chat::valid_id(chat_id) {
        let _ = std::fs::remove_dir_all(dir(root).join(chat_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_session_is_read_back_only_by_its_own_chat_source_and_model() {
        let root = std::env::temp_dir().join(format!("worker-session-{}", uuid::Uuid::new_v4()));
        let chat = uuid::Uuid::new_v4().to_string();
        let key = format!("{PREFIX}{chat}:deepseek:flash");
        let history = vec![json!({"role":"user","content":"x"}), json!({"role":"assistant","content":"y"})];
        assert!(load(&root, &key).is_empty());
        save(&root, &key, &history);
        assert_eq!(load(&root, &key), history);
        assert!(load(&root, &format!("{PREFIX}{chat}:other:flash")).is_empty());
        assert!(load(&root, &format!("{PREFIX}{chat}:deepseek:pro")).is_empty());
        assert!(load(&root, &format!("{PREFIX}{}:deepseek:flash", uuid::Uuid::new_v4())).is_empty());
        remove_chat(&root, &chat);
        assert!(load(&root, &key).is_empty());
        // A key that is not a chat uuid never becomes a path.
        save(&root, &format!("{PREFIX}../evil:a:b"), &history);
        assert!(!root.join("evil").exists() && std::fs::read_dir(dir(&root)).map_or(0, |entries| entries.count()) == 0);
        let _ = std::fs::remove_dir_all(&root);
    }
}
