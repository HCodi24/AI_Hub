use crate::providers::cli_support;
use std::{ffi::OsString, process::Stdio, time::Duration};

/// One `codegraph` init/sync at a time; a second chat on the same folder waits and then finds nothing to do.
static SYNC_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const SYNC_TIMEOUT: Duration = Duration::from_secs(25);
/// The first index of a big project can take minutes.
const INIT_TIMEOUT: Duration = Duration::from_secs(300);
/// Log lines starting with this text are failures: the chat opens its activity list by itself to show them.
const FAILED: &str = "CodeGraph không dùng được";

/// Runs one `codegraph` command in `folder`. The error is a one-line reason that is shown to the user.
async fn run_codegraph(path: &std::path::Path, folder: &std::path::Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let arguments = args.iter().map(OsString::from).collect::<Vec<_>>();
    let mut command = cli_support::command_for(path, &arguments);
    command.current_dir(folder).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    crate::core::process::configure_hidden(&mut command);
    let name = args.first().copied().unwrap_or_default();
    match crate::core::process::run_with_timeout(&mut command, limit).await {
        Ok(output) if output.status.success() => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        Ok(output) => {
            let detail = String::from_utf8_lossy(&output.stderr);
            let detail = detail.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default();
            Err(format!("codegraph {name} lỗi {}: {}", output.status, crate::core::text::truncate_chars(detail.trim(), 200)))
        }
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => Err(format!("codegraph {name} chạy quá {} giây", limit.as_secs())),
        Err(error) => Err(format!("không chạy được codegraph {name}: {error}")),
    }
}

/// What the app prepared for a session's first request.
#[derive(Default)]
pub struct Prepared {
    pub indexed: bool,
    pub synced: bool,
    /// Lines for the chat's activity log.
    pub notes: Vec<String>,
}

/// Builds the index when the project has none, otherwise syncs it. The AI asks its own questions through the
/// codegraph MCP tool. Never fails the chat: every problem becomes a note and the AI is told what is missing.
pub async fn prepare(folder: &std::path::Path) -> Prepared {
    let mut out = Prepared::default();
    if !folder.is_dir() {
        return out;
    }
    let database = folder.join(".codegraph/codegraph.db");
    let Some(path) = cli_support::resolve_executable("codegraph", None) else {
        out.indexed = database.is_file();
        out.notes.push(format!("{FAILED}: chưa cài CLI codegraph (Cài đặt › Tiện ích)"));
        return out;
    };
    {
        let _guard = SYNC_LOCK.lock().await;
        if !database.is_file() {
            match run_codegraph(&path, folder, &["init", "-y"], INIT_TIMEOUT).await {
                Ok(_) => { out.synced = true; out.notes.push("CodeGraph: đã lập chỉ mục dự án".into()); }
                Err(reason) => out.notes.push(format!("{FAILED}: không lập được chỉ mục ({reason})")),
            }
        } else {
            match run_codegraph(&path, folder, &["sync", "-q"], SYNC_TIMEOUT).await {
                Ok(_) => out.synced = true,
                Err(reason) => out.notes.push(format!("{FAILED}: không đồng bộ được chỉ mục, dùng bản cũ ({reason})")),
            }
        }
    }
    out.indexed = database.is_file();
    out
}
