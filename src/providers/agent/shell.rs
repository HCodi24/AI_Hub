use super::super::{emit_text, ProviderEvent};
use serde_json::{json, Value};
use std::{ffi::OsStr, path::Path, process::Stdio, time::Duration};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

pub(super) fn openai_definition() -> Value {
    let tool = anthropic_definition();
    json!({
        "type":"function",
        "function":{
            "name":tool["name"],
            "description":tool["description"],
            "parameters":tool["input_schema"]
        }
    })
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

async fn kill_process_tree(pid: Option<u32>) {
    crate::core::process::kill_process_tree(pid).await;
    #[cfg(windows)]
    if let Some(pid) = pid {
        terminate_windows_process_tree(pid);
    }
    #[cfg(unix)]
    if let Some(pid) = pid {
        let mut command = Command::new("kill");
        command
            .args(["-TERM", &format!("-{pid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::core::process::configure_hidden(&mut command);
        let _ = command.status().await;
    }
}

#[cfg(windows)]
fn terminate_windows_process_tree(root: u32) {
    use std::collections::HashSet;
    use std::ffi::c_void;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        process_id: u32,
        default_heap_id: usize,
        module_id: u32,
        thread_count: u32,
        parent_process_id: u32,
        base_priority: i32,
        flags: u32,
        executable: [u16; 260],
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> *mut c_void;
        fn Process32FirstW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: *mut c_void, entry: *mut ProcessEntry32W) -> i32;
        fn OpenProcess(access: u32, inherit: i32, process_id: u32) -> *mut c_void;
        fn TerminateProcess(process: *mut c_void, exit_code: u32) -> i32;
        fn CloseHandle(handle: *mut c_void) -> i32;
    }

    let snapshot = unsafe { CreateToolhelp32Snapshot(0x00000002, 0) };
    if snapshot.is_null() || snapshot as isize == -1 {
        return;
    }
    let mut processes = Vec::new();
    unsafe {
        let mut entry = std::mem::zeroed::<ProcessEntry32W>();
        entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        if Process32FirstW(snapshot, &mut entry) != 0 {
            loop {
                processes.push((entry.process_id, entry.parent_process_id));
                entry.size = std::mem::size_of::<ProcessEntry32W>() as u32;
                if Process32NextW(snapshot, &mut entry) == 0 {
                    break;
                }
            }
        }
        CloseHandle(snapshot);
    }
    let mut seen = HashSet::from([root]);
    let mut pending = vec![root];
    let mut tree = Vec::new();
    while let Some(parent) = pending.pop() {
        tree.push(parent);
        for (process, process_parent) in &processes {
            if *process_parent == parent && seen.insert(*process) {
                pending.push(*process);
            }
        }
    }
    for process in tree.into_iter().rev() {
        unsafe {
            let handle = OpenProcess(0x0001, 0, process);
            if !handle.is_null() {
                TerminateProcess(handle, 1);
                CloseHandle(handle);
            }
        }
    }
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
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn runs_command_and_reports_exit_code() {
        let root = std::env::current_dir().unwrap();
        let command = if cfg!(windows) {
            "Write-Output 'agent-output'; exit 7"
        } else {
            "printf 'agent-output\\n'; exit 7"
        };
        let output = run_shell(&root, command, 10, &CancellationToken::new()).await;
        assert!(output.starts_with("exit code 7\n"), "{output}");
        assert!(output.contains("agent-output"), "{output}");
    }

    #[tokio::test]
    async fn timeout_kills_the_process_and_returns_an_error() {
        let root = std::env::current_dir().unwrap();
        let command = if cfg!(windows) {
            "Start-Sleep -Seconds 10"
        } else {
            "sleep 10"
        };
        let output = run_shell(&root, command, 1, &CancellationToken::new()).await;
        assert!(
            output.starts_with("error: Lệnh hết thời gian sau 1 giây"),
            "{output}"
        );
    }

    #[tokio::test]
    async fn cancellation_kills_the_process_and_returns_an_error() {
        let root = std::env::current_dir().unwrap();
        let command = if cfg!(windows) {
            "Start-Sleep -Seconds 10"
        } else {
            "sleep 10"
        };
        let cancel = CancellationToken::new();
        let task = tokio::spawn({
            let cancel = cancel.clone();
            async move { run_shell(&root, command, 30, &cancel).await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
        assert_eq!(task.await.unwrap(), "error: Đã dừng");
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn timeout_kills_descendant_processes() {
        let marker_path =
            std::env::temp_dir().join(format!("ai-hub-agent-{}.txt", uuid::Uuid::new_v4()));
        let marker = marker_path.to_string_lossy().replace('\'', "''");
        let command = format!(
            "Start-Process powershell -WindowStyle Hidden -ArgumentList '-NoProfile -NonInteractive -Command \"Start-Sleep -Seconds 2; Set-Content -LiteralPath ''{marker}'' late\"'; Start-Sleep -Seconds 10"
        );
        let root = std::env::current_dir().unwrap();
        let _ = run_shell(&root, &command, 1, &CancellationToken::new()).await;
        tokio::time::sleep(Duration::from_millis(2500)).await;
        let descendant_survived = marker_path.exists();
        let _ = std::fs::remove_file(marker_path);
        assert!(!descendant_survived);
    }

    #[test]
    fn long_output_keeps_the_tail() {
        let output = tail(&format!("{}tail", "x".repeat(20_000)), 20_000);
        assert!(output.starts_with("...[output truncated; showing last 20000 characters]"));
        assert!(output.ends_with("tail"));
        assert!(output.chars().count() <= 20_100);
    }

    #[test]
    fn timeout_defaults_and_caps_at_six_hundred_seconds() {
        assert_eq!(timeout_secs(&json!({})), 120);
        assert_eq!(timeout_secs(&json!({"timeout_secs":900})), 600);
        assert_eq!(timeout_secs(&json!({"timeout_secs":0})), 1);
    }

    #[test]
    fn credential_environment_names_are_removed() {
        for name in [
            "OPENAI_API_KEY",
            "SERVICE_TOKEN",
            "DB_PASSWORD",
            "AUTH_HEADER",
        ] {
            assert!(sensitive_env_name(OsStr::new(name)));
        }
        assert!(!sensitive_env_name(OsStr::new("PATH")));
    }
}
