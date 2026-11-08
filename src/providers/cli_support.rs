use super::ProviderEvent;
use anyhow::{anyhow, Context, Result};
use std::{
    collections::VecDeque,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, Command},
    sync::mpsc,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

pub fn resolve_executable(name: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    for suffix in [".exe", ".cmd", ""] {
        let candidate = format!("{name}{suffix}");
        for directory in std::env::split_paths(&paths) {
            let path = directory.join(&candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}

pub fn command_for(path: &Path, args: &[OsString]) -> Command {
    let is_cmd = path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("cmd"));
    let mut command = if cfg!(windows) && is_cmd {
        // Keep user values out of cmd.exe's command text. Each argument is placed in
        // the child environment and expanded inside its own quoted argument.
        let mut command = Command::new("cmd");
        command.args(["/D", "/S", "/C"]);
        let mut command_line = String::from("\"\"%AI_HUB_CLI_EXECUTABLE%\"");
        for (index, _) in args.iter().enumerate() {
            command_line.push_str(&format!(" \"%AI_HUB_CLI_ARG_{index}%\""));
        }
        command_line.push('"');
        append_cmd_line(&mut command, &command_line);
        command.env("AI_HUB_CLI_EXECUTABLE", path);
        for (index, argument) in args.iter().enumerate() {
            command.env(format!("AI_HUB_CLI_ARG_{index}"), argument);
        }
        command
    } else {
        let mut command = Command::new(path);
        command.args(args);
        command
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    command
}

#[cfg(windows)]
fn append_cmd_line(command: &mut Command, command_line: &str) {
    use std::os::windows::process::CommandExt;
    command.as_std_mut().raw_arg(command_line);
}

#[cfg(not(windows))]
fn append_cmd_line(command: &mut Command, command_line: &str) {
    command.arg(command_line);
}

pub struct JsonlCommand<'a> {
    pub cli_name: &'static str,
    pub executable_name: &'static str,
    pub missing_message: &'static str,
    pub folder: &'a str,
    pub args: Vec<OsString>,
    pub prompt: String,
}

pub async fn run_jsonl(
    command_spec: JsonlCommand<'_>,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
    mut parse_line: impl FnMut(&str) -> Vec<ProviderEvent> + Send,
) -> Result<()> {
    let path = resolve_executable(command_spec.executable_name)
        .ok_or_else(|| anyhow!(command_spec.missing_message))?;
    let cli_name = command_spec.cli_name;
    let mut command = command_for(&path, &command_spec.args);
    command
        .current_dir(command_spec.folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .with_context(|| format!("Khong the khoi dong {cli_name}"))?;

    let Some(stdout) = child.stdout.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} khong mo duoc stdout"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} khong mo duoc stderr"));
    };
    let stderr_task = tokio::spawn(collect_stderr(stderr));
    let Some(stdin) = child.stdin.take() else {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Err(anyhow!("{cli_name} khong mo duoc stdin"));
    };

    let write_result = tokio::select! {
        _ = cancel.cancelled() => {
            terminate_child_tree(&mut child).await;
            let _ = stderr_task.await;
            return Ok(());
        }
        result = write_prompt(stdin, command_spec.prompt) => result,
    };
    if let Err(error) = write_result {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Err(error).with_context(|| format!("Khong the gui prompt toi {cli_name}"));
    }

    let mut stdout = BufReader::new(stdout).lines();
    let mut was_cancelled = false;
    'read: loop {
        let line = tokio::select! {
            _ = cancel.cancelled() => {
                was_cancelled = true;
                break 'read;
            }
            result = stdout.next_line() => match result {
                Ok(line) => line,
                Err(error) => {
                    terminate_child_tree(&mut child).await;
                    let _ = stderr_task.await;
                    return Err(error).context("Khong the doc stdout cua CLI");
                }
            },
        };
        let Some(line) = line else { break };
        for event in parse_line(&line) {
            tokio::select! {
                _ = cancel.cancelled() => {
                    was_cancelled = true;
                    break 'read;
                }
                result = events.send(event) => {
                    if result.is_err() {
                        terminate_child_tree(&mut child).await;
                        let _ = stderr_task.await;
                        return Ok(());
                    }
                }
            }
        }
    }
    if was_cancelled {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Ok(());
    }

    let wait_result = tokio::select! {
        _ = cancel.cancelled() => {
            terminate_child_tree(&mut child).await;
            let _ = stderr_task.await;
            return Ok(());
        }
        result = child.wait() => result,
    };
    let status = match wait_result {
        Ok(status) => status,
        Err(error) => {
            terminate_child_tree(&mut child).await;
            let _ = stderr_task.await;
            return Err(error).with_context(|| format!("Khong the doi {cli_name} ket thuc"));
        }
    };
    let stderr_lines = stderr_task.await.unwrap_or_default();
    if !status.success() {
        let stderr = stderr_lines.join("\n");
        let status = status
            .code()
            .map(|code| format!("ma thoat {code}"))
            .unwrap_or_else(|| "bi dung boi he thong".to_owned());
        let message = if stderr.is_empty() {
            format!("{cli_name} ket thuc voi {status}.")
        } else {
            format!("{cli_name} ket thuc voi {status}:\n{stderr}")
        };
        let _ = events.send(ProviderEvent::text("error", message)).await;
    }
    Ok(())
}

pub fn compose_prompt(messages: &[crate::chat::ChatMessage], has_session: bool) -> String {
    let Some(latest) = messages.last() else {
        return String::new();
    };
    if has_session || messages.len() == 1 {
        return latest.content.clone();
    }

    let history = &messages[..messages.len() - 1];
    let start = history.len().saturating_sub(8);
    let mut transcript = String::from("Earlier conversation for context:\n");
    for message in &history[start..] {
        let role = match message.role.as_str() {
            "assistant" => "Assistant",
            "system" => "System",
            _ => "User",
        };
        let content = truncate_chars(message.content.trim(), 700);
        transcript.push_str(role);
        transcript.push_str(": ");
        transcript.push_str(&content);
        transcript.push('\n');
        if transcript.chars().count() >= 4_000 {
            break;
        }
    }
    transcript.push_str("\nLatest user message:\n");
    transcript.push_str(&latest.content);
    transcript
}

pub fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

pub fn safe_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && !session_id.starts_with('-')
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

pub fn one_line(value: &str, max_chars: usize) -> String {
    truncate_chars(&value.split_whitespace().collect::<Vec<_>>().join(" "), max_chars)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let mut result = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        result.push('…');
    }
    result
}

async fn write_prompt(mut stdin: ChildStdin, prompt: String) -> std::io::Result<()> {
    stdin.write_all(prompt.as_bytes()).await?;
    stdin.shutdown().await
}

async fn collect_stderr(stderr: ChildStderr) -> Vec<String> {
    let mut lines = BufReader::new(stderr).lines();
    let mut tail = VecDeque::with_capacity(20);
    while let Ok(Some(line)) = lines.next_line().await {
        if tail.len() == 20 {
            tail.pop_front();
        }
        tail.push_back(truncate_chars(&line, 2_000));
    }
    tail.into_iter().collect()
}

async fn terminate_child_tree(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.wait().await;
        return;
    }
    #[cfg(windows)]
    {
        if let Some(pid) = child.id() {
            let mut taskkill = Command::new("taskkill");
            taskkill
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            taskkill.as_std_mut().creation_flags(CREATE_NO_WINDOW);
            if !matches!(taskkill.status().await, Ok(status) if status.success()) {
                let _ = child.kill().await;
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill().await;
    }
    let _ = timeout(Duration::from_secs(3), child.wait()).await;
}

