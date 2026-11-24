use crate::providers::cli_support;
use anyhow::{bail, Result};
use serde_json::Value;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// agy's `input_tokens` only ever grows (it does not drop after a compaction), so the size the budget is checked against is the growth since
/// the last compaction: `(raw input, raw input right after the last compaction)` per conversation, kept while the app runs.
pub(super) fn sizes() -> &'static std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>> {
    static SIZES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>>> = std::sync::OnceLock::new();
    SIZES.get_or_init(Default::default)
}

pub(super) fn note_input(conversation: &str, input: u64) {
    let mut sizes = sizes().lock().unwrap_or_else(|e| e.into_inner());
    let base = sizes.get(conversation).map_or(0, |(_, base)| *base);
    sizes.insert(conversation.to_owned(), (input, base));
    crate::providers::codex::compact::note_size(conversation, input.saturating_sub(base));
}

/// Compacts a saved conversation with agy's own `/compact` (same conversation id afterwards). Used before a turn past the token budget
/// and by the "Nén ngữ cảnh" button.
pub(crate) async fn compact_session(provider: &crate::config::ProviderConfig, home: &std::path::Path, folder: &str, conversation: &str, cancel: &CancellationToken) -> Result<()> {
    if !cli_support::safe_session_id(conversation) {
        bail!("Antigravity conversation id không hợp lệ");
    }
    let outcome = std::sync::Arc::new(std::sync::Mutex::new(None::<u64>));
    let seen = outcome.clone();
    let (sink, mut drained) = mpsc::channel(64);
    tokio::spawn(async move { while drained.recv().await.is_some() {} });
    cli_support::run_jsonl(
        cli_support::JsonlCommand {
            config_dir: Some(home.to_owned()),
            cli_name: "Antigravity CLI",
            executable_name: "agy",
            missing_message: "Chưa cài Antigravity CLI: irm https://antigravity.google/cli/install.ps1 | iex",
            configured_executable: provider.executable.clone(),
            folder,
            args: ["--print=/compact", "--conversation", conversation, "--output-format", "json"].map(OsString::from).into(),
            prompt: String::new(),
            watchdog_minutes: None,
            interact: None,
        },
        sink,
        cancel.clone(),
        move |line| {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if value["status"] == "SUCCESS" { *seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(value["usage"]["input_tokens"].as_u64().unwrap_or(0)); }
            }
            Vec::new()
        },
    )
    .await?;
    let Some(input) = *outcome.lock().unwrap_or_else(|e| e.into_inner()) else { bail!("Antigravity không báo nén thành công") };
    sizes().lock().unwrap_or_else(|e| e.into_inner()).insert(conversation.to_owned(), (input, input));
    crate::providers::codex::compact::forget(conversation);
    Ok(())
}
