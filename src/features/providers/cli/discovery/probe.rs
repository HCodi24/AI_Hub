use super::resolve_executable;
use crate::providers::cli_support;
use std::{ffi::OsString, path::Path, process::Stdio, time::Duration};
use tokio::time::timeout;

pub(crate) async fn run_cli_version(path: &Path, limit: Duration) -> Option<String> {
    let mut command = cli_support::command_for(path, &[OsString::from("--version")]);
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("claude.exe"))
        || path.to_string_lossy().to_lowercase().contains("claude")
    {
        cli_support::clean_claude_environment(&mut command);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(limit, command.output()).await.ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    (!value.is_empty()).then(|| value.chars().take(240).collect())
}

pub(crate) async fn probe_cli_info(
    name: &str,
    explicit_path: &str,
) -> (bool, Option<String>, Option<String>) {
    let Some(path) = resolve_executable(name, Some(explicit_path.trim())) else {
        return (false, None, None);
    };
    let path_text = path.to_string_lossy().into_owned();
    let version = run_cli_version(&path, Duration::from_secs(5)).await;
    (true, version, Some(path_text))
}
