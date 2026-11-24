use super::jsonl_command;
use crate::providers::{cli_support, codex};
use anyhow::{bail, Result};
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Compacts a saved Claude session with its own `/compact` (same session id afterwards). Used before a turn past the token
/// budget and by the "Nén ngữ cảnh" button. Verified live: the session keeps its facts after it.
pub(crate) async fn compact_session(provider: &crate::config::ProviderConfig, home: &std::path::Path, folder: &str, native: &str, model: &str, cancel: &CancellationToken) -> Result<()> {
    if !cli_support::safe_session_id(native) {
        bail!("Claude session id không hợp lệ");
    }
    let mut args = vec![OsString::from("-p"), "--output-format".into(), "stream-json".into(), "--verbose".into(), "--permission-mode".into(), "plan".into()];
    if !model.is_empty() && cli_support::valid_model(model) {
        args.extend([OsString::from("--model"), OsString::from(model)]);
    }
    args.extend([OsString::from("--resume"), OsString::from(native)]);
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = done.clone();
    let (sink, mut drained) = mpsc::channel(64);
    tokio::spawn(async move { while drained.recv().await.is_some() {} });
    cli_support::run_jsonl(
        jsonl_command(provider, home.to_owned(), folder, args, "/compact".to_owned(), None),
        sink,
        cancel.clone(),
        move |line| {
            if line.contains("\"compact_result\":\"success\"") {
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            Vec::new()
        },
    )
    .await?;
    if !done.load(std::sync::atomic::Ordering::SeqCst) {
        bail!("Claude không báo nén thành công");
    }
    codex::compact::forget(native);
    Ok(())
}
