use super::ProviderEvent;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::{collections::VecDeque, env, ffi::OsString, path::Path, process::Stdio, time::Duration};

pub use crate::core::text::one_line;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, Command},
    sync::mpsc,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

pub use super::discovery::resolve_executable;
#[cfg(test)]
pub use super::discovery::version_dir_key;

pub fn environment_names_to_remove(names: &[String]) -> Vec<String> {
    names
        .iter()
        .filter(|name| {
            let upper = name.to_ascii_uppercase();
            upper.starts_with("CLAUDE") || upper.starts_with("ANTHROPIC_")
        })
        .cloned()
        .collect()
}

pub fn parse_auth_logged_in(output: &[u8]) -> Option<bool> {
    let value: Value = serde_json::from_slice(output).ok()?;
    value["loggedIn"].as_bool()
}

pub fn clean_claude_environment(command: &mut Command) {
    let names = env::vars().map(|(name, _)| name).collect::<Vec<_>>();
    for name in environment_names_to_remove(&names) {
        command.env_remove(name);
    }
    // AI Hub's Claude Code CLI gets its own config dir (login, settings, history), separate from the user's
    // `~/.claude` used by the Claude desktop app, so the two can be signed in to different accounts.
    if let Some(dir) = ai_hub_claude_config_dir() {
        command.env("CLAUDE_CONFIG_DIR", dir);
    }
}

/// `<AI Hub data dir>\claude-config` (created on demand), e.g. `%APPDATA%\ai-hub\claude-config`.
pub fn ai_hub_claude_config_dir() -> Option<std::path::PathBuf> {
    let config_path = crate::core::config::config_file_path().ok()?;
    let dir = crate::core::paths::data_dir(&config_path)
        .ok()?
        .join("claude-config");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

pub fn searched_locations(name: &str, explicit_path_was_set: bool) -> String {
    let mut locations = Vec::new();
    if explicit_path_was_set {
        locations.push("configured file path");
    }
    locations.push("process PATH");
    #[cfg(windows)]
    {
        locations.push("user/machine registry PATH");
        locations.push("%APPDATA%\\npm");
        locations.push("%LOCALAPPDATA%\\Programs\\nodejs");
        locations.push("%USERPROFILE%\\.local\\bin");
        if name.eq_ignore_ascii_case("codex") {
            locations.push("%LOCALAPPDATA%\\OpenAI\\Codex\\bin\\*");
        }
    }
    #[cfg(not(windows))]
    let _ = name;
    locations.join(", ")
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

        if is_cmd {
            let mut prepend = Vec::new();
            if let Some(parent) = path.parent() {
                prepend.push(parent.to_path_buf());
            }
            if let Some(node) = resolve_executable("node", None).filter(|node| {
                node.extension().is_some_and(|extension| {
                    extension.to_string_lossy().eq_ignore_ascii_case("exe")
                })
            }) {
                if let Some(parent) = node.parent() {
                    prepend.push(parent.to_path_buf());
                }
            }
            if let Some(existing) = env::var_os("PATH") {
                prepend.extend(env::split_paths(&existing));
            }
            if let Ok(path) = env::join_paths(prepend) {
                command.env("PATH", path);
            }
        }
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
    pub configured_executable: String,
    pub folder: &'a str,
    pub args: Vec<OsString>,
    pub prompt: String,
    pub watchdog_minutes: Option<u32>,
}

pub async fn run_jsonl(
    command_spec: JsonlCommand<'_>,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
    mut parse_line: impl FnMut(&str) -> Vec<ProviderEvent> + Send,
) -> Result<()> {
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
        return Ok(());
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
    }
    command
        .current_dir(folder)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .with_context(|| format!("Không thể khởi động {cli_name}"))?;

    let Some(stdout) = child.stdout.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} không thể mở stdout"));
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_child_tree(&mut child).await;
        return Err(anyhow!("{cli_name} không thể mở stderr"));
    };
    let stderr_task = tokio::spawn(collect_stderr(stderr));
    let Some(stdin) = child.stdin.take() else {
        terminate_child_tree(&mut child).await;
        let _ = stderr_task.await;
        return Err(anyhow!("{cli_name} không thể mở stdin"));
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
        return Err(error).with_context(|| format!("Không thể gửi nội dung đến {cli_name}"));
    }

    let mut stdout = BufReader::new(stdout).lines();
    let mut was_cancelled = false;
    let watchdog = command_spec
        .watchdog_minutes
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
                    terminate_child_tree(&mut child).await;
                    let message=format!("Codex không phản hồi sau {} phút — đã dừng", command_spec.watchdog_minutes.unwrap_or_default());
                    let _=events.send(ProviderEvent::text("error",message)).await;
                    let _=events.send(ProviderEvent::text("watchdog_kill","watchdog_kill")).await;
                    let _=stderr_task.await;
                    return Ok(());
                }
                Err(error) => {
                    terminate_child_tree(&mut child).await;
                    let _ = stderr_task.await;
                    return Err(error).context("Không thể đọc stdout của CLI");
                }
                Ok(line) => line,
            },
        };
        let line = line_result;
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
            return Err(error).with_context(|| format!("Không thể chờ {cli_name} kết thúc"));
        }
    };
    let stderr_lines = stderr_task.await.unwrap_or_default();
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
    Ok(())
}

pub fn token_usage_log(usage: &Value, total_cost_usd: Option<&Value>) -> Option<String> {
    let input_tokens = usage["input_tokens"].as_u64()?;
    let output_tokens = usage["output_tokens"].as_u64()?;
    let cached_tokens = usage["cached_input_tokens"].as_u64().unwrap_or_else(|| {
        usage["cache_read_input_tokens"]
            .as_u64()
            .unwrap_or_default()
            .saturating_add(
                usage["cache_creation_input_tokens"]
                    .as_u64()
                    .unwrap_or_default(),
            )
    });
    let reasoning_tokens = usage["reasoning_output_tokens"]
        .as_u64()
        .or_else(|| usage["thinking_tokens"].as_u64());
    let mut line = format!("Token: vào {input_tokens} (cache {cached_tokens}), ra {output_tokens}");
    if let Some(reasoning_tokens) = reasoning_tokens {
        line.push_str(&format!(" (suy luận {reasoning_tokens})"));
    }
    if let Some(cost) = total_cost_usd.filter(|value| !value.is_null()) {
        let cost = format_cost(cost);
        line.push_str(&format!(", chi phí ${cost}"));
    }
    Some(line)
}

#[cfg(test)]
mod cost_tests {
    use super::token_usage_log;
    use serde_json::json;

    #[test]
    fn costs_round_to_four_decimals_and_trim_trailing_zeros() {
        for (value, expected) in [
            (json!(0.5748474000000001), "$0.5748"),
            (json!("0.5748474000000001"), "$0.5748"),
            (json!(1.2300), "$1.23"),
            (json!("2.00000"), "$2"),
            (json!(0), "$0"),
            (json!(0.00001), "$0"),
            (json!("0.99999"), "$1"),
        ] {
            let usage = json!({"input_tokens": 12, "output_tokens": 3});
            let line = token_usage_log(&usage, Some(&value)).unwrap();
            assert_eq!(
                line,
                format!("Token: vào 12 (cache 0), ra 3, chi phí {expected}")
            );
        }
    }
}

fn format_cost(cost: &Value) -> String {
    let number = cost.as_f64().or_else(|| cost.as_str()?.parse::<f64>().ok());
    if let Some(number) = number.filter(|number| number.is_finite()) {
        let rounded = format!("{number:.4}");
        return rounded
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned();
    }
    cost.as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| cost.to_string())
}

/// Which AI wrote a reply, as `ChatMessage::provider` labels it.
fn ai_label(kind: &str) -> &'static str {
    match kind {
        "codex_cli" => "Codex",
        "claude_cli" | "anthropic" => "Claude",
        "deepseek" => "DeepSeek",
        _ => "AI",
    }
}

const TRANSCRIPT_MESSAGES: usize = 30;
const TRANSCRIPT_CHARS: usize = 40_000;
/// The answer right before the user's new message matters most, so it keeps (almost) everything.
const LAST_ANSWER_CHARS: usize = 12_000;
const OLDER_MESSAGE_CHARS: usize = 2_500;

/// Newest messages first until the budget is spent, printed oldest first. Done once, when a CLI has no session
/// for this chat (a source switch), so a larger budget costs little.
fn transcript(messages: &[crate::chat::ChatMessage]) -> String {
    let mut lines = Vec::new();
    let mut used = 0;
    for (age, message) in messages.iter().rev().filter(|m| !m.content.trim().is_empty()).enumerate() {
        if age >= TRANSCRIPT_MESSAGES || used >= TRANSCRIPT_CHARS {
            break;
        }
        let limit = if age == 0 && message.role == "assistant" { LAST_ANSWER_CHARS } else { OLDER_MESSAGE_CHARS };
        let who = match message.role.as_str() {
            "assistant" => format!("Assistant ({})", ai_label(&message.provider)),
            "system" => "System".to_owned(),
            _ => "User".to_owned(),
        };
        let line = format!("{who}: {}\n", crate::core::text::truncate_chars(message.content.trim(), limit));
        used += line.chars().count();
        lines.push(line);
    }
    lines.reverse();
    lines.concat()
}

/// Prompt for a CLI of `kind` (`codex_cli`, `claude_cli`):
/// - no session yet (first use, or the user switched to this source): a transcript of the conversation so far;
/// - session resumed but another AI answered since this CLI's last reply: only what it missed;
/// - otherwise just the latest message (the CLI remembers the rest).
pub fn compose_prompt_for(messages: &[crate::chat::ChatMessage], has_session: bool, kind: &str) -> String {
    let Some(latest) = messages.last() else {
        return String::new();
    };
    if messages.len() == 1 {
        return latest.content.clone();
    }
    let history = &messages[..messages.len() - 1];
    if has_session {
        // Old chats carry no author labels: assume the session knows everything.
        let Some(seen) = history.iter().rposition(|m| m.role == "assistant" && m.provider == kind) else {
            return latest.content.clone();
        };
        let missed = &history[seen + 1..];
        if !missed.iter().any(|m| m.role == "assistant" && !m.content.trim().is_empty()) {
            return latest.content.clone();
        }
        return format!(
            "While you were away, the conversation continued with other AIs:\n{}\nLatest user message:\n{}",
            transcript(missed),
            latest.content
        );
    }
    format!(
        "Earlier conversation for context (some answers come from other AIs):\n{}\nLatest user message:\n{}",
        transcript(history),
        latest.content
    )
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
        tail.push_back(crate::core::text::truncate_chars(&line, 2_000));
    }
    tail.into_iter().collect()
}

pub(crate) async fn terminate_child_tree(child: &mut Child) {
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

#[cfg(test)]
mod claude_support_tests {
    use super::{environment_names_to_remove, parse_auth_logged_in, version_dir_key};

    #[test]
    fn removes_claude_and_anthropic_environment_names() {
        let names = [
            "CLAUDECODE",
            "CLAUDE_CODE_HOME",
            "ANTHROPIC_BASE_URL",
            "PATH",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert_eq!(environment_names_to_remove(&names), names[..3]);
    }

    #[test]
    fn sorts_claude_version_directories_numerically() {
        let older = version_dir_key("2.1.309").unwrap();
        let newer = version_dir_key("2.1.317").unwrap();
        assert!(newer > older);
        assert!(version_dir_key("not-a-version").is_none());
    }

    #[test]
    fn parses_auth_status_logged_in_boolean() {
        assert_eq!(
            parse_auth_logged_in(br#"{"loggedIn":true,"authMethod":"oauth"}"#),
            Some(true)
        );
        assert_eq!(parse_auth_logged_in(br#"{"loggedIn":false}"#), Some(false));
        assert_eq!(parse_auth_logged_in(b"{}"), None);
        assert_eq!(parse_auth_logged_in(b"not json"), None);
    }
}

#[cfg(test)]
mod prompt_tests {
    use super::*;
    use crate::chat::ChatMessage;

    fn message(role: &str, provider: &str, content: &str) -> ChatMessage {
        serde_json::from_value(serde_json::json!({"role": role, "content": content, "provider": provider})).unwrap()
    }

    #[test]
    fn a_fresh_session_gets_the_long_last_answer_and_who_wrote_it() {
        let long = "x".repeat(5_000);
        let chat = [message("user", "", "q1"), message("assistant", "codex_cli", &long), message("user", "", "what did you see?")];
        let prompt = compose_prompt_for(&chat, false, "claude_cli");
        assert!(prompt.contains("Assistant (Codex): ") && prompt.contains(&long));
        assert!(prompt.ends_with("Latest user message:\nwhat did you see?"));
    }

    #[test]
    fn a_resumed_session_only_hears_what_other_ais_said_meanwhile() {
        let chat = [
            message("user", "", "q1"),
            message("assistant", "claude_cli", "a1"),
            message("user", "", "q2"),
            message("assistant", "codex_cli", "codex answer"),
            message("user", "", "q3"),
        ];
        let prompt = compose_prompt_for(&chat, true, "claude_cli");
        assert!(prompt.contains("User: q2") && prompt.contains("Assistant (Codex): codex answer"));
        assert!(!prompt.contains("a1") && prompt.ends_with("q3"));
        // Nobody else spoke, or old chat without labels: just the latest message.
        assert_eq!(compose_prompt_for(&chat, true, "codex_cli"), "q3");
        assert_eq!(compose_prompt_for(&chat[..3], true, "claude_cli"), "q2");
        assert_eq!(compose_prompt_for(&chat, true, "deepseek"), "q3");
    }
}
