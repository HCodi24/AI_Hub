use super::{
    cli_label,
    parse::{allowed_login_url, extract_https_url, push_tail, CLAUDE_DOMAINS, GOOGLE_DOMAINS},
    LoginStartResponse,
};
use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
    providers,
};
use std::{
    ffi::OsString,
    path::Path,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
    time::timeout,
};
use uuid::Uuid;

pub(crate) async fn login_provider_start(
    state: &AppState,
    id: &str,
    provider: &ProviderConfig,
    dir: &Path,
) -> ProviderResult<LoginStartResponse> {
    let label = cli_label(&provider.kind);
    let agy = provider.kind == "antigravity_cli";
    let (cli, domains) = if agy {
        ("agy", GOOGLE_DOMAINS)
    } else {
        ("claude", CLAUDE_DOMAINS)
    };
    let path = providers::cli_support::resolve_executable(cli, Some(&provider.executable))
        .ok_or_else(|| {
            let message = if agy {
                "Không tìm thấy Antigravity CLI (agy)"
            } else {
                "Không tìm thấy Claude Code CLI"
            };
            ProviderError::new(ProviderErrorKind::NotFound, message)
        })?;
    if let Some(mut old) = state.claude_login.lock().await.take() {
        let _ = old.child.kill().await;
    }
    let args: Vec<OsString> = if agy {
        ["acp", "--login"].map(OsString::from).into()
    } else {
        ["auth", "login", "--claudeai"].map(OsString::from).into()
    };
    let mut command = providers::cli_support::command_for(&path, &args);
    if agy {
        // A web account keeps its sign-in in its own home, apart from the machine's (see `cli_support::agy_environment`).
        providers::antigravity_cli::sync_login_mode(provider);
        let _ = std::fs::remove_file(dir.join(providers::antigravity_cli::SIGNED_OUT_FILE));
        providers::cli_support::agy_environment(&mut command, dir);
        command.current_dir(dir);
    } else {
        providers::cli_support::clean_claude_environment(&mut command);
        command.env("CLAUDE_CONFIG_DIR", dir).current_dir(dir);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|e| ProviderError::new(ProviderErrorKind::Internal, e.to_string()))?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("Không mở được stdin {label}"),
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("Không mở được stdout {label}"),
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ProviderError::new(
            ProviderErrorKind::Internal,
            format!("Không mở được stderr {label}"),
        )
    })?;
    let (tx, rx) = mpsc::channel(32);
    tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).await.is_err() {
                break;
            }
        }
    });
    let stderr_tail = std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let stderr_store = stderr_tail.clone();
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut tail = stderr_store.lock().await;
            push_tail(&mut tail, &line);
        }
    });
    let mut session = crate::app::ClaudeLoginSession {
        id: Uuid::new_v4().to_string(),
        provider_id: id.to_owned(),
        child,
        stdin,
        stdout: rx,
        output_tail: Vec::new(),
        stderr_tail,
        last_activity: Instant::now(),
    };
    let url = timeout(Duration::from_secs(20), async {
        loop {
            let line = session.stdout.recv().await?;
            push_tail(&mut session.output_tail, &line);
            if let Some(url) = extract_https_url(&line) {
                return Some(url.to_owned());
            }
        }
    })
    .await
    .ok()
    .flatten()
    .ok_or_else(|| {
        let _ = session.child.start_kill();
        ProviderError::new(
            ProviderErrorKind::GatewayTimeout,
            format!("{label} không trả link đăng nhập trong 20 giây"),
        )
    })?;
    if !allowed_login_url(&url, domains) {
        let _ = session.child.start_kill();
        return Err(ProviderError::new(
            ProviderErrorKind::BadGateway,
            "CLI trả link đăng nhập không được phép",
        ));
    }
    session.last_activity = Instant::now();
    let result = LoginStartResponse {
        session_id: session.id.clone(),
        url,
    };
    *state.claude_login.lock().await = Some(session);
    Ok(result)
}
