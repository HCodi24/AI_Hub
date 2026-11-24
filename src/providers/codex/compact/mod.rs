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
mod tests;
