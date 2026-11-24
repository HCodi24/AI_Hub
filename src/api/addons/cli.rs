use super::bad;
use crate::{api::ApiResult, providers::cli_support};
use std::{ffi::OsString, process::Stdio, time::Duration};

/// Runs a CLI with the supplied config home, if any, and returns the last 4000
/// characters of its combined output.
pub(super) async fn run_cli_in(
    path: &std::path::Path,
    args: &[&str],
    claude: bool,
    codex_home: Option<&std::path::Path>,
) -> ApiResult<String> {
    let text = run_cli_full(path, args, claude, codex_home).await?;
    Ok(last_chars(&text, 4000))
}

/// Same, with the whole output: a JSON listing is cut and unreadable at 4000 characters.
pub(super) async fn run_cli_full(
    path: &std::path::Path,
    args: &[&str],
    claude: bool,
    codex_home: Option<&std::path::Path>,
) -> ApiResult<String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut command = cli_support::command_for(path, &args);
    if claude {
        cli_support::clean_claude_environment(&mut command);
    }
    if let Some(home) = codex_home {
        if path.file_stem().is_some_and(|stem| stem.eq_ignore_ascii_case("agy")) {
            cli_support::agy_environment(&mut command, home);
        } else {
            command.env(if claude { "CLAUDE_CONFIG_DIR" } else { "CODEX_HOME" }, home);
        }
    }
    crate::core::process::configure_hidden(&mut command);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(180), command.output())
        .await
        .map_err(|_| bad("Lệnh chạy quá 3 phút và đã bị dừng"))?
        .map_err(|error| bad(format!("Không chạy được lệnh: {error}")))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text = text.trim().to_owned();
    if !output.status.success() {
        return Err(bad(if text.is_empty() {
            "Lệnh thất bại".to_owned()
        } else {
            last_chars(&text, 4000)
        }));
    }
    Ok(text)
}

pub(super) fn last_chars(text: &str, count: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    chars[chars.len().saturating_sub(count)..].iter().collect()
}
