//! Context compaction of a Codex thread (`thread/compact/start`): Codex summarises its own history in place, so a long
//! thread keeps going instead of being dropped and rebuilt. Developer instructions and the rules in the first message
//! survived it in the live test below.
use super::transport::Client;
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{collections::HashMap, path::Path, sync::{Mutex, OnceLock}, time::Duration};
use tokio_util::sync::CancellationToken;

/// Input tokens of the last model call per native thread id: how big the thread is now. Lives as long as the app.
fn sizes() -> &'static Mutex<HashMap<String, u64>> {
    static SIZES: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    SIZES.get_or_init(Mutex::default)
}

pub(crate) fn note_size(thread_id: &str, tokens: u64) {
    sizes().lock().unwrap_or_else(|e| e.into_inner()).insert(thread_id.to_owned(), tokens);
}

pub(crate) fn forget(thread_id: &str) {
    sizes().lock().unwrap_or_else(|e| e.into_inner()).remove(thread_id);
}

/// The size the thread had at its last model call, when it is at least `budget` (0 = never compact).
pub(crate) fn needs_compaction(thread_id: &str, budget: u64) -> Option<u64> {
    let size = sizes().lock().unwrap_or_else(|e| e.into_inner()).get(thread_id).copied()?;
    (budget > 0 && size >= budget).then_some(size)
}

pub(crate) fn thread_token_budget(config_path: &Path) -> u64 {
    std::fs::read(config_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<crate::config::Config>(&bytes).ok())
        .map_or(0, |config| config.thread_token_budget)
}

/// Compacts the thread and waits until Codex has finished. The caller starts its own turn afterwards.
pub(super) async fn compact(client: &mut Client, thread_id: &str, cancel: &CancellationToken) -> Result<()> {
    client.call("thread/compact/start", json!({"threadId":thread_id})).await?;
    let wait = async {
        loop {
            let value = tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                value = client.next() => value?,
            };
            if value.get("id").is_some() && value.get("method").is_some() {
                client.reject_request(&value).await?;
                continue;
            }
            match value["method"].as_str().unwrap_or("") {
                "turn/completed" => return Ok(()),
                "error" if value["params"]["willRetry"] != true => bail!("Codex không nén được luồng: {}", value["params"]["error"]["message"].as_str().unwrap_or("")),
                _ => {}
            }
        }
    };
    let result = tokio::time::timeout(Duration::from_secs(300), wait).await.context("Codex nén luồng quá lâu")?;
    forget(thread_id);
    result
}

/// Compacts a saved thread outside any chat turn (the user's "Nén ngữ cảnh" button): resumes it in a fresh app-server, compacts, closes.
pub(crate) async fn compact_saved(executable: &Path, home: &Path, folder: &Path, thread_id: &str) -> Result<()> {
    let mut client = Client::start(executable, home, folder).await?;
    let result = async {
        client.call("thread/resume", json!({"threadId":thread_id,"cwd":folder,"approvalPolicy":"never","sandbox":"read-only","selectedCapabilityRoots":[]})).await?;
        compact(&mut client, thread_id, &CancellationToken::new()).await
    }.await;
    client.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thread_is_compacted_only_once_it_is_over_the_budget() {
        note_size("t-small", 90_000);
        note_size("t-big", 160_000);
        assert_eq!(needs_compaction("t-small", 150_000), None);
        assert_eq!(needs_compaction("t-big", 150_000), Some(160_000));
        assert_eq!(needs_compaction("t-big", 0), None, "0 = off");
        assert_eq!(needs_compaction("t-unknown", 150_000), None, "a thread the app has not measured is left alone");
        // A later, smaller reading (after the thread was compacted) replaces the old one.
        note_size("t-big", 40_000);
        assert_eq!(needs_compaction("t-big", 150_000), None);
    }

    #[test]
    fn the_budget_comes_from_the_config_file() {
        let dir = std::env::temp_dir().join(format!("mutis-compact-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.json");
        assert_eq!(thread_token_budget(&path), 0, "no file: compaction stays off");
        std::fs::write(&path, br#"{"thread_token_budget":123000}"#).unwrap();
        assert_eq!(thread_token_budget(&path), 123_000);
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(thread_token_budget(&path), 0, "unreadable file: compaction stays off");
        std::fs::remove_dir_all(&dir).ok();
    }

    async fn turn(client: &mut Client, id: &str, text: &str) -> String {
        client.call("turn/start", json!({"threadId":id,"effort":"low","input":[{"type":"text","text":text}]})).await.unwrap();
        let mut reply = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
        loop {
            let value = tokio::time::timeout_at(deadline, client.next()).await.expect("took too long").unwrap();
            if value["method"] == "item/completed" && value["params"]["item"]["type"] == "agentMessage" {
                reply = value["params"]["item"]["text"].as_str().unwrap_or("").to_owned();
            }
            if value["method"] == "turn/completed" { return reply; }
        }
    }

    /// Real Codex: a second client resumes the thread (as the app does on the next message), the app's `compact` runs, and the
    /// thread still knows a fact from before and a rule from the first message. Uses the app's signed-in Codex home.
    #[tokio::test]
    #[ignore = "Needs MUTIS_CODEX_TEST_EXE and the app's signed-in Codex home; runs real turns"]
    async fn live_a_resumed_thread_keeps_its_memory_after_compaction() {
        let exe = std::env::var_os("MUTIS_CODEX_TEST_EXE").expect("Set MUTIS_CODEX_TEST_EXE");
        let home = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("ai-hub").join("codex").join("codex-cli");
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let params = json!({"cwd":folder,"model":"gpt-6.2-sol","approvalPolicy":"untrusted","sandbox":"read-only","selectedCapabilityRoots":[],
            "developerInstructions":"Rule R1: end every reply with the word BANANA."});
        let mut first = Client::start(Path::new(&exe), &home, &folder).await.unwrap();
        let thread = first.call("thread/start", params.clone()).await.unwrap();
        let id = thread["thread"]["id"].as_str().unwrap().to_owned();
        turn(&mut first, &id, "MutiS rule R2 (for the whole session): put the word MANGO right before BANANA. Write 150 words on why the sky is blue. Remember the secret code 4417.").await;
        first.close().await;

        // What the "Nén ngữ cảnh" button runs, in a client of its own.
        compact_saved(Path::new(&exe), &home, &folder, &id).await.unwrap();
        let mut second = Client::start(Path::new(&exe), &home, &folder).await.unwrap();
        let mut resume = params;
        resume["threadId"] = json!(id);
        second.call("thread/resume", resume).await.unwrap();
        let reply = turn(&mut second, &id, "What was the secret code? Answer in one line, following rules R1 and R2.").await;
        second.close().await;
        assert!(reply.contains("4417"), "the fact was lost: {reply}");
        assert!(reply.trim_end().ends_with("MANGO BANANA"), "the rules were lost: {reply}");
    }
}
