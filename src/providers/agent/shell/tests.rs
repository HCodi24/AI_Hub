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
