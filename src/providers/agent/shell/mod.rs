use crate::providers::{emit_text, ProviderEvent};
use serde_json::{json, Value};
use std::{ffi::OsStr, path::Path, process::Stdio, time::Duration};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

mod kill;

use self::kill::kill_process_tree;

pub(super) fn openai_definition() -> Value {
    super::openai_tool(anthropic_definition())
}

pub(super) fn anthropic_definition() -> Value {
    json!({
        "name":"run_command",
        "description":"Run a command in the chat working folder. Prefer small, safe commands.",
        "input_schema":{
            "type":"object",
            "properties":{
                "command":{"type":"string"},
                "timeout_secs":{"type":"integer","minimum":1,"maximum":600}
            },
            "required":["command"]
        }
    })
}

pub(super) async fn run_command(
    root: &Path,
    args: &Value,
    events: &tokio::sync::mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> String {
    let Some(command) = args["command"].as_str() else {
        return "error: Thiếu command".into();
    };
    let timeout_secs = timeout_secs(args);
    let display = command.chars().take(200).collect::<String>();
    let _ = emit_text(events, "log", format!("Lệnh: {display}")).await;
    run_shell(root, command, timeout_secs, cancel).await
}

fn timeout_secs(args: &Value) -> u64 {
    args["timeout_secs"].as_u64().unwrap_or(120).clamp(1, 600)
}

async fn run_shell(
    root: &Path,
    command: &str,
    timeout_secs: u64,
    cancel: &CancellationToken,
) -> String {
    let mut shell = if cfg!(windows) {
        let mut shell = Command::new("powershell");
        shell
            .args(["-NoProfile", "-NonInteractive", "-Command"])
            .arg(format!(
                "[Console]::OutputEncoding=[Text.Encoding]::UTF8; {command}"
            ));
        shell
    } else {
        let mut shell = Command::new("sh");
        shell.args(["-lc", command]);
        shell
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        shell.as_std_mut().process_group(0);
    }
    shell
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .env_clear();
    // Keep command discovery working while excluding environment variables commonly used for credentials.
    for (name, value) in std::env::vars_os().filter(|(name, _)| !sensitive_env_name(name)) {
        shell.env(name, value);
    }
    crate::core::process::configure_hidden(&mut shell);
    let child = match shell.spawn() {
        Ok(child) => child,
        Err(error) => return format!("error: Không thể chạy lệnh: {error}"),
    };
    let pid = child.id();
    let mut output_task = tokio::spawn(child.wait_with_output());
    let output = tokio::select! {
        _ = cancel.cancelled() => {
            kill_process_tree(pid).await;
            output_task.abort();
            let _ = output_task.await;
            return "error: Đã dừng".into();
        }
        output = tokio::time::timeout(Duration::from_secs(timeout_secs), &mut output_task) => output,
    };
    let output = match output {
        Err(_) => {
            kill_process_tree(pid).await;
            output_task.abort();
            let _ = output_task.await;
            return format!("error: Lệnh hết thời gian sau {timeout_secs} giây");
        }
        Ok(Err(error)) => return format!("error: {error}"),
        Ok(Ok(Err(error))) => return format!("error: {error}"),
        Ok(Ok(Ok(output))) => output,
    };
    let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }
    let exit_code = output.status.code().unwrap_or(-1);
    let output = crate::core::text::strip_ansi(&combined);
    let output = tail(&output, 20_000);
    format!("exit code {exit_code}\n{output}")
}

fn sensitive_env_name(name: &OsStr) -> bool {
    let name = name.to_string_lossy().to_ascii_lowercase();
    ["key", "token", "secret", "password", "credential", "auth"]
        .iter()
        .any(|needle| name.contains(needle))
}

fn tail(value: &str, max_chars: usize) -> String {
    let count = value.chars().count();
    if count <= max_chars {
        return value.to_owned();
    }
    let tail = value.chars().skip(count - max_chars).collect::<String>();
    format!("...[output truncated; showing last {max_chars} characters]\n{tail}")
}

#[cfg(test)]
mod tests;
