use super::ProviderEvent;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::{
    collections::VecDeque,
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStderr, ChildStdin, Command},
    sync::mpsc,
    time::timeout,
};
use tokio_util::sync::CancellationToken;

pub fn resolve_executable(name: &str, explicit_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = explicit_path.map(str::trim).filter(|path| !path.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_file() && safe_cli_executable(&path) {
            return Some(path);
        }
    }

    let names = [
        format!("{name}.exe"),
        format!("{name}.cmd"),
        name.to_owned(),
    ];
    let process_path = env::var_os("PATH").into_iter().flat_map(|value| {
        env::split_paths(&value)
            .filter(|path| valid_path_entry(path))
            .collect::<Vec<_>>()
    });
    if let Some(path) = find_in_directories(&names, process_path) {
        return Some(path);
    }

    #[cfg(windows)]
    {
        if let Some(path) = find_in_directories(&names, registry_path_directories()) {
            return Some(path);
        }

        let directories = well_known_directories();
        if let Some(path) = find_in_directories(&names, directories) {
            return Some(path);
        }

        if name.eq_ignore_ascii_case("codex") {
            if let Some(path) = newest_codex_install() {
                return Some(path);
            }
        }
        if name.eq_ignore_ascii_case("claude") {
            if let Some(path) = newest_claude_app_install() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(windows)]
fn newest_claude_app_install() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("LOCALAPPDATA")?).join("Claude-Data");
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|install| {
            if !install.file_type().ok()?.is_dir() {
                return None;
            }
            let versions = install.path().join("claude-code");
            Some(
                std::fs::read_dir(versions)
                    .ok()?
                    .filter_map(Result::ok)
                    .filter_map(|version| {
                        if !version.file_type().ok()?.is_dir() {
                            return None;
                        }
                        let version_name = version.file_name().to_string_lossy().into_owned();
                        let parsed = version_dir_key(&version_name)?;
                        let hashes = std::fs::read_dir(version.path()).ok()?;
                        hashes.filter_map(Result::ok).find_map(|hash| {
                            let executable = hash.path().join("claude.exe");
                            (hash.file_type().ok()?.is_dir()
                                && executable.is_file()
                                && safe_cli_executable(&executable))
                            .then_some((parsed.clone(), executable))
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect::<Vec<_>>();
    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    candidates.into_iter().next().map(|(_, path)| path)
}

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

pub fn version_dir_key(version: &str) -> Option<(Vec<u64>, bool, String)> {
    let (release, prerelease) = version.split_once('-').unwrap_or((version, ""));
    let mut numbers = Vec::new();
    for part in release.split('.') {
        numbers.push(part.parse::<u64>().ok()?);
    }
    if numbers.len() != 3 || version.contains('+') || prerelease.contains('+') {
        return None;
    }
    Some((numbers, prerelease.is_empty(), prerelease.to_owned()))
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

fn valid_path_entry(path: &Path) -> bool {
    let path = path.to_string_lossy();
    !path.trim().is_empty() && !path.contains('$') && !path.contains('%')
}

fn find_in_directories(
    names: &[String],
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let directories = directories
        .into_iter()
        .filter(|directory| valid_path_entry(directory))
        .collect::<Vec<_>>();
    for name in names {
        for directory in &directories {
            let candidate = directory.join(name);
            if candidate.is_file() && safe_cli_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn safe_cli_executable(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.contains("\\program files\\windowsapps\\") {
        return false;
    }
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Claude.exe"))
    {
        return !path
            .parent()
            .is_some_and(|parent| parent.join("resources").join("app.asar").exists());
    }
    true
}

#[cfg(windows)]
fn registry_path_directories() -> Vec<PathBuf> {
    [
        r"HKCU\Environment",
        r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    ]
    .into_iter()
    .filter_map(read_registry_path)
    .flat_map(|path| env::split_paths(std::ffi::OsStr::new(&path)).collect::<Vec<_>>())
    .filter(|path| valid_path_entry(path))
    .collect()
}

#[cfg(windows)]
fn read_registry_path(key: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = StdCommand::new("reg")
        .args(["query", key, "/v", "Path"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let name = fields.next()?;
        let value_type = fields.next()?;
        if !name.eq_ignore_ascii_case("Path") || !matches!(value_type, "REG_SZ" | "REG_EXPAND_SZ") {
            return None;
        }
        let value_start = line.find(value_type)? + value_type.len();
        let value = line[value_start..].trim();
        Some(expand_windows_variables(value))
    })
}

#[cfg(windows)]
fn expand_windows_variables(value: &str) -> String {
    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };
        let variable = &after_start[..end];
        if let Some(value) = env::var_os(variable) {
            expanded.push_str(&value.to_string_lossy());
        } else {
            expanded.push('%');
            expanded.push_str(variable);
            expanded.push('%');
        }
        rest = &after_start[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

#[cfg(windows)]
fn well_known_directories() -> Vec<PathBuf> {
    [
        env::var_os("APPDATA").map(|path| PathBuf::from(path).join("npm")),
        env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Programs").join("nodejs")),
        env::var_os("USERPROFILE").map(|path| PathBuf::from(path).join(".local").join("bin")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(windows)]
fn newest_codex_install() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("LOCALAPPDATA")?)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let executable = entry.path().join("codex.exe");
            if !executable.is_file() || !safe_cli_executable(&executable) {
                return None;
            }
            let modified = executable
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            Some((modified, executable))
        })
        .collect::<Vec<(SystemTime, PathBuf)>>();
    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    candidates.into_iter().next().map(|(_, path)| path)
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
            result = async { if let Some(duration)=watchdog { timeout(duration, read_line).await.map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "watchdog"))? } else { read_line.await } } => match result {
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
        let cost = cost
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| cost.to_string());
        line.push_str(&format!(", chi phí ${cost}"));
    }
    Some(line)
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
    truncate_chars(
        &value.split_whitespace().collect::<Vec<_>>().join(" "),
        max_chars,
    )
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
