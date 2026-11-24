use super::{
    command::{command_for, terminate_child_tree},
    env::{agy_environment, clean_claude_environment},
    resolve_executable, searched_locations,
};
use crate::providers::ProviderEvent;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::{collections::VecDeque, ffi::OsString, process::{ExitStatus, Stdio}, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines},
    process::{Child, ChildStderr, ChildStdin, ChildStdout},
    sync::mpsc,
    task::JoinHandle,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

/// What a source that can be asked in the middle of a run does with one of its output lines.
pub enum Reaction {
    /// Answer at once (a tool that is always fine, or never available here).
    Reply(String),
    /// Put `shown` to the user; `reply` turns their yes/no into the line written back. A "no" also offers `need` one level up.
    Ask { shown: String, need: &'static str, reply: Box<dyn Fn(bool) -> String + Send> },
}
pub type Interact = Box<dyn Fn(&str) -> Option<Reaction> + Send + Sync>;

pub struct JsonlCommand<'a> {
    pub config_dir: Option<std::path::PathBuf>,
    pub cli_name: &'static str,
    pub executable_name: &'static str,
    pub missing_message: &'static str,
    pub configured_executable: String,
    pub folder: &'a str,
    pub args: Vec<OsString>,
    pub prompt: String,
    pub watchdog_minutes: Option<u32>,
    /// Two-way sources: stdin stays open until the result, and these lines are answered as they come.
    pub interact: Option<Interact>,
}

pub async fn run_jsonl(
    command_spec: JsonlCommand<'_>,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
    mut parse_line: impl FnMut(&str) -> Vec<ProviderEvent> + Send,
) -> Result<()> {
    let cli_name = command_spec.cli_name;
    let Some(mut child) = spawn_child(&command_spec, &events).await? else {
        return Ok(());
    };

    let Some(stdout) = child.stdout.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} không thể mở stdout"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} không thể mở stderr"));
    };
    let mut stderr_task = tokio::spawn(collect_stderr(stderr));
    let Some(stdin) = child.stdin.take() else {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Err(anyhow!("{cli_name} không thể mở stdin"));
    };

    let mut live = None;
    let write_result = tokio::select! {
        _ = cancel.cancelled() => {
            terminate_child_tree(&mut child).await;
            let _ = stderr_task.await;
            return Ok(());
        }
        result = write_prompt(stdin, command_spec.prompt, command_spec.interact.is_some()) => result,
    };
    if let Err(error) = write_result.map(|open| live = open) {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Err(error).with_context(|| format!("Không thể gửi nội dung đến {cli_name}"));
    }

    let mut stdout = BufReader::new(stdout).lines();
    if !read_stdout(
        &mut stdout,
        &mut child,
        &mut live,
        &mut stderr_task,
        &events,
        &cancel,
        command_spec.interact.as_ref(),
        command_spec.watchdog_minutes,
        &mut parse_line,
    )
    .await?
    {
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
            return Err(error).with_context(|| format!("Không thể chờ {cli_name} kết thúc"));
        }
    };
    let stderr_lines = stderr_task.await.unwrap_or_default();
    report_exit(status, stderr_lines, cli_name, &events).await;
    Ok(())
}

async fn spawn_child(
    command_spec: &JsonlCommand<'_>,
    events: &mpsc::Sender<ProviderEvent>,
) -> Result<Option<Child>> {
    let cli_name = command_spec.cli_name;
    let folder = tokio::fs::canonicalize(command_spec.folder)
        .await
        .map(crate::core::text::strip_verbatim)
        .with_context(|| format!("Không thể giải quyết thư mục làm việc cho {cli_name}"))?;
    let folder_text = folder.to_string_lossy();
    if folder_text.contains('%') || folder_text.contains('"') {
        let _ = events
            .send(ProviderEvent::text(
                "error",
                "Đường dẫn thư mục làm việc không được chứa ký tự % hoặc dấu ngoặc kép.",
            ))
            .await;
        return Ok(None);
    }
    let configured_executable = command_spec.configured_executable.trim();
    let path = resolve_executable(command_spec.executable_name, Some(configured_executable))
        .ok_or_else(|| {
            anyhow!(
                "{} (đã tìm: {})",
                command_spec.missing_message,
                searched_locations(
                    command_spec.executable_name,
                    !configured_executable.is_empty()
                )
            )
        })?;
    let mut command = command_for(&path, &command_spec.args);
    if command_spec.executable_name.eq_ignore_ascii_case("claude") {
        clean_claude_environment(&mut command);
        if let Some(dir) = &command_spec.config_dir {
            command.env("CLAUDE_CONFIG_DIR", dir);
        }
    }
    if command_spec.executable_name.eq_ignore_ascii_case("agy") {
        if let Some(dir) = &command_spec.config_dir {
            agy_environment(&mut command, dir);
        }
    }
    command
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command
        .spawn()
        .with_context(|| format!("Không thể khởi động {cli_name}"))?;
    Ok(Some(child))
}

async fn read_stdout(
    stdout: &mut Lines<BufReader<ChildStdout>>,
    child: &mut Child,
    live: &mut Option<ChildStdin>,
    stderr_task: &mut JoinHandle<Vec<String>>,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
    interact: Option<&Interact>,
    watchdog_minutes: Option<u32>,
    parse_line: &mut (impl FnMut(&str) -> Vec<ProviderEvent> + Send),
) -> Result<bool> {
    let mut was_cancelled = false;
    let watchdog = watchdog_minutes
        .map(|minutes| Duration::from_secs(u64::from(minutes) * 60));
    'read: loop {
        let read_line = stdout.next_line();
        let line_result = tokio::select! {
            _ = cancel.cancelled() => {
                was_cancelled = true;
                break 'read;
            }
            result = async {
                if let Some(duration) = watchdog {
                    timeout(duration, read_line)
                        .await
                        .map_err(|_| {
                            std::io::Error::new(std::io::ErrorKind::TimedOut, "watchdog")
                        })?
                } else {
                    read_line.await
                }
            } => match result {
                Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
                    terminate_child_tree(child).await;
                    let message=format!("Codex không phản hồi sau {} phút — đã dừng", watchdog_minutes.unwrap_or_default());
                    let _=events.send(ProviderEvent::text("error",message)).await;
                    let _=events.send(ProviderEvent::text("watchdog_kill","watchdog_kill")).await;
                    let _=stderr_task.await;
                    return Ok(false);
                }
                Err(error) => {
                    terminate_child_tree(child).await;
                    let _ = stderr_task.await;
                    return Err(error).context("Không thể đọc stdout của CLI");
                }
                Ok(line) => line,
            },
        };
        let line = line_result;
        let Some(line) = line else { break };
        if let Some(interact) = interact {
            match interact(&line) {
                Some(Reaction::Reply(text)) => { write_reply(live, &text).await; continue; }
                Some(Reaction::Ask { shown, need, reply }) => {
                    let Some(allowed) = super::approvals::confirm(events, cancel, shown).await else { was_cancelled = true; break 'read };
                    let note = if allowed { "Bạn đã cho phép bước này một lần." } else { "Bạn đã từ chối bước này." };
                    let _ = events.send(ProviderEvent::text("log", note.to_owned())).await;
                    if !allowed {
                        let _ = events.send(ProviderEvent { name: "permission_denied", data: serde_json::json!({ "need": need }) }).await;
                    }
                    write_reply(live, &reply(allowed)).await;
                    continue;
                }
                None => {}
            }
            // The turn is over: closing stdin lets the CLI exit.
            if serde_json::from_str::<Value>(&line).is_ok_and(|value| value["type"] == "result") { *live = None; }
        }
        for event in parse_line(&line) {
            tokio::select! {
                _ = cancel.cancelled() => {
                    was_cancelled = true;
                    break 'read;
                }
                result = events.send(event) => {
                    if result.is_err() {
                        terminate_child_tree(child).await;
                        let _ = stderr_task.await;
                        return Ok(false);
                    }
                }
            }
        }
    }
    if was_cancelled {
        terminate_child_tree(child).await;
        let _ = stderr_task.await;
        return Ok(false);
    }
    Ok(true)
}

async fn report_exit(
    status: ExitStatus,
    stderr_lines: Vec<String>,
    cli_name: &str,
    events: &mpsc::Sender<ProviderEvent>,
) {
    if !status.success() {
        let stderr = stderr_lines.join("\n");
        let status = status
            .code()
            .map(|code| format!("với mã thoát {code}"))
            .unwrap_or_else(|| "bị dừng bởi hệ thống".to_owned());
        let message = if stderr.is_empty() {
            format!("{cli_name} kết thúc {status}.")
        } else {
            format!("{cli_name} kết thúc {status}:\n{stderr}")
        };
        let _ = events.send(ProviderEvent::text("error", message)).await;
    }
}

/// Sends the prompt; `keep_open` leaves stdin for later answers, otherwise it is closed so the CLI starts at once.
async fn write_prompt(mut stdin: ChildStdin, prompt: String, keep_open: bool) -> std::io::Result<Option<ChildStdin>> {
    stdin.write_all(prompt.as_bytes()).await?;
    if keep_open {
        stdin.flush().await?;
        return Ok(Some(stdin));
    }
    stdin.shutdown().await?;
    Ok(None)
}

async fn write_reply(stdin: &mut Option<ChildStdin>, line: &str) {
    if let Some(stdin) = stdin {
        let _ = stdin.write_all(format!("{line}
").as_bytes()).await;
        let _ = stdin.flush().await;
    }
}

async fn collect_stderr(stderr: ChildStderr) -> Vec<String> {
    let mut lines = BufReader::new(stderr).lines();
    let mut tail = VecDeque::with_capacity(20);
    while let Ok(Some(line)) = lines.next_line().await {
        if tail.len() == 20 {
            tail.pop_front();
        }
        tail.push_back(crate::core::text::truncate_chars(&line, 2_000));
    }
    tail.into_iter().collect()
}
