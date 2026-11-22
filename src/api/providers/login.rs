//! Claude Code CLI and Antigravity (web account) sign-in: start the CLI, hand back the sign-in link, then send it the confirmation code.
use super::connection::{connection, test_provider_uncached};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsString,
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::mpsc,
    time::timeout,
};
use uuid::Uuid;

#[derive(Serialize)]
pub(crate) struct LoginStartResponse {
    session_id: String,
    url: String,
}
#[derive(Deserialize)]
pub(crate) struct LoginCodeInput {
    session_id: String,
    code: String,
}
#[derive(Serialize)]
pub(crate) struct LoginCodeResponse {
    ok: bool,
    logged_in: bool,
    message: String,
    output_tail: String,
}

fn extract_https_url(text: &str) -> Option<&str> {
    let start = text.find("https://")?;
    let candidate = &text[start..];
    let end = candidate
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(candidate.len());
    Some(candidate[..end].trim_end_matches(['.', ',', ';', ')', ']', '"', '\'']))
}
fn allowed_login_url(value: &str, domains: &[&str]) -> bool {
    reqwest::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
            && url.host_str().is_some_and(|host| domains.iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}"))))
    })
}
const CLAUDE_DOMAINS: &[&str] = &["claude.com", "claude.ai", "anthropic.com"];
const GOOGLE_DOMAINS: &[&str] = &["accounts.google.com"];
/// `Logged in as a@b.c.` (agy's last line after a web sign-in).
fn logged_in_email(lines: &[String]) -> Option<String> {
    lines.iter().rev().find_map(|line| {
        let rest = line.trim().strip_prefix("Logged in as ")?;
        let email = rest.trim_end_matches('.').trim();
        email.contains('@').then(|| email.to_owned())
    })
}
fn validate_login_code(code: &str) -> Result<String, &'static str> {
    if code.chars().count() > 512 {
        return Err("Mã xác nhận dài quá 512 ký tự");
    }
    if code.chars().any(char::is_control) {
        return Err("Mã xác nhận không được chứa ký tự điều khiển");
    }
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return Err("Hãy nhập mã xác nhận");
    }
    Ok(trimmed.to_owned())
}
async fn login_provider_config(state: &AppState, id: &str) -> ApiResult<ProviderConfig> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn AI"))?;
    if provider.kind != "claude_cli" && !(provider.kind == "antigravity_cli" && provider.web_login) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ Claude Code CLI và tài khoản Web của Antigravity mới đăng nhập được",
        ));
    }
    Ok(provider)
}

pub(crate) async fn login_provider_start(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<LoginStartResponse>> {
    let provider = login_provider_config(&state, &id).await?;
    let agy = provider.kind == "antigravity_cli";
    let (cli, domains) = if agy { ("agy", GOOGLE_DOMAINS) } else { ("claude", CLAUDE_DOMAINS) };
    let dir = if agy {
        providers::antigravity_cli::clean_home(&provider.id).map_err(super::claude::error)?
    } else {
        super::claude::ensure_login_mode(&provider, true)?;
        super::claude::ensure_idle(&state).await?;
        let dir = providers::claude::environment::home(&state.config_path, &provider)
            .map_err(super::claude::error)?;
        super::claude::invalidate(&state).await?;
        dir
    };
    let path = providers::cli_support::resolve_executable(cli, Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, if agy { "Không tìm thấy Antigravity CLI (agy)" } else { "Không tìm thấy Claude Code CLI" }))?;
    if let Some(mut old) = state.claude_login.lock().await.take() {
        let _ = old.child.kill().await;
    }
    let args: Vec<OsString> = if agy { ["acp", "--login"].map(OsString::from).into() } else { ["auth", "login", "--claudeai"].map(OsString::from).into() };
    let mut command = providers::cli_support::command_for(&path, &args);
    if agy {
        // A web account keeps its sign-in in its own home, apart from the machine's (see `cli_support::agy_environment`).
        providers::antigravity_cli::sync_login_mode(&provider);
        let _ = std::fs::remove_file(dir.join(providers::antigravity_cli::SIGNED_OUT_FILE));
        providers::cli_support::agy_environment(&mut command, &dir);
        command.current_dir(&dir);
    } else {
        providers::cli_support::clean_claude_environment(&mut command);
        command.env("CLAUDE_CONFIG_DIR", &dir).current_dir(&dir);
        #[cfg(windows)]
        command.env("BROWSER", "cmd /c rem");
        #[cfg(not(windows))]
        command.env("BROWSER", "true");
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command
        .spawn()
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không mở được stdin Claude CLI",
        )
    })?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không mở được stdout Claude CLI",
        )
    })?;
    let stderr = child.stderr.take().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không mở được stderr Claude CLI",
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
            if tail.len() == 20 {
                tail.remove(0);
            }
            tail.push(crate::core::text::truncate_chars(&line, 2_000));
        }
    });
    let mut session = crate::app::ClaudeLoginSession {
        id: Uuid::new_v4().to_string(),
        provider_id: id.clone(),
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
            if session.output_tail.len() == 20 {
                session.output_tail.remove(0);
            }
            session
                .output_tail
                .push(crate::core::text::truncate_chars(&line, 2_000));
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
        ApiError::new(
            StatusCode::GATEWAY_TIMEOUT,
            "Claude CLI không trả link đăng nhập trong 20 giây",
        )
    })?;
    if !allowed_login_url(&url, domains) {
        let _ = session.child.start_kill();
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "CLI trả link đăng nhập không được phép",
        ));
    }
    session.last_activity = Instant::now();
    let result = LoginStartResponse {
        session_id: session.id.clone(),
        url,
    };
    *state.claude_login.lock().await = Some(session);
    state.provider_status.lock().await.remove(&id);
    Ok(Json(result))
}

pub(crate) async fn login_provider_code(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<LoginCodeInput>,
) -> ApiResult<Json<LoginCodeResponse>> {
    let provider = login_provider_config(&state, &id).await?;
    let code =
        validate_login_code(&input.code).map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e))?;
    let mut sessions = state.claude_login.lock().await;
    let session = sessions
        .as_mut()
        .filter(|s| s.id == input.session_id && s.provider_id == id)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "Phiên đăng nhập đã hết hạn hoặc không tồn tại",
            )
        })?;
    session.last_activity = Instant::now();
    session
        .stdin
        .write_all(format!("{code}\n").as_bytes())
        .await
        .map_err(|e| {
            ApiError::new(
                StatusCode::BAD_GATEWAY,
                format!("Không gửi được mã xác nhận: {e}"),
            )
        })?;
    let exit = timeout(Duration::from_secs(30), session.child.wait()).await;
    let (exited, message) = match &exit {
        Ok(Ok(status)) => (
            true,
            if status.success() {
                "Claude CLI đã hoàn tất đăng nhập".to_owned()
            } else {
                format!("Claude CLI kết thúc với mã {}", status.code().unwrap_or(-1))
            },
        ),
        Ok(Err(e)) => (true, format!("Không chờ được Claude CLI: {e}")),
        Err(_) => (false, "Claude CLI chưa kết thúc sau 30 giây".to_owned()),
    };
    let session = sessions.as_mut().expect("session present");
    while let Ok(line) = session.stdout.try_recv() {
        if session.output_tail.len() == 20 {
            session.output_tail.remove(0);
        }
        session
            .output_tail
            .push(crate::core::text::truncate_chars(&line, 2_000));
    }
    let email = logged_in_email(&session.output_tail);
    let mut lines = session.output_tail.clone();
    lines.extend(session.stderr_tail.lock().await.iter().cloned());
    let output_tail = lines
        .into_iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
        .replace(&code, "[đã ẩn mã]");
    if exited {
        sessions.take();
    }
    drop(sessions);
    let status = if exited {
        state.provider_status.lock().await.remove(&id);
        test_provider_uncached(State(state.clone()), Path(id))
            .await?
            .0
    } else {
        connection(false, "error", &message, "")
    };
    let logged_in = status.status == "connected";
    if logged_in && provider.kind == "antigravity_cli" {
        if let (Some(email), Ok(home)) = (email, providers::antigravity_cli::clean_home(&provider.id)) {
            let _ = std::fs::write(home.join(super::antigravity::EMAIL_FILE), email);
            // The status was read before the email was saved; drop it so the next read shows the account.
            state.provider_status.lock().await.remove(&provider.id);
        }
    } else if logged_in {
        if let Ok(home) = providers::claude::environment::home(&state.config_path, &provider) {
            providers::cli_support::set_auth_origin(&home, Some("web"));
        }
    }
    Ok(Json(LoginCodeResponse {
        ok: exited && logged_in,
        logged_in,
        message: if logged_in {
            if provider.kind == "antigravity_cli" { "Đã đăng nhập Antigravity".into() } else { "Đã đăng nhập Claude Code CLI".into() }
        } else {
            message.clone()
        },
        output_tail,
    }))
}
pub(crate) async fn login_provider_cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    login_provider_config(&state, &id).await?;
    let mut active = state.claude_login.lock().await;
    if active
        .as_ref()
        .is_some_and(|session| session.provider_id == id)
    {
        if let Some(mut session) = active.take() {
            let _ = session.child.kill().await;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod claude_login_tests {
    use super::{allowed_login_url, extract_https_url, logged_in_email, validate_login_code, CLAUDE_DOMAINS, GOOGLE_DOMAINS};

    #[test]
    fn extracts_login_url_from_cli_intro_and_prompt_output() {
        let output = concat!(
            "Opening browser to sign in…\n",
            "If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=abc",
            "&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&state=xyz\n",
            "Paste code here if prompted >"
        );
        let url = extract_https_url(output).unwrap();
        assert!(url.starts_with("https://claude.com/cai/oauth/authorize?"));
        assert!(allowed_login_url(url, CLAUDE_DOMAINS));
    }

    #[test]
    fn antigravity_sign_in_allows_only_google_accounts_and_reads_the_email() {
        assert!(allowed_login_url("https://accounts.google.com/o/oauth2/auth?client_id=x", GOOGLE_DOMAINS));
        assert!(!allowed_login_url("https://claude.com/login", GOOGLE_DOMAINS));
        assert!(!allowed_login_url("https://accounts.google.com.evil.example/", GOOGLE_DOMAINS));
        let lines = vec!["Then paste the code".to_owned(), "Logged in as a.b@gmail.com.".to_owned()];
        assert_eq!(logged_in_email(&lines).as_deref(), Some("a.b@gmail.com"));
        assert_eq!(logged_in_email(&["Logged in as nobody".to_owned()]), None);
    }

    #[test]
    fn allows_only_claude_and_anthropic_hosts() {
        for host in ["claude.com", "accounts.claude.ai", "platform.anthropic.com"] {
            assert!(allowed_login_url(&format!("https://{host}/oauth"), CLAUDE_DOMAINS));
        }
        for url in [
            "http://claude.com/login",
            "https://evilclaude.com/login",
            "https://example.com/?next=claude.com",
            "https://claude.ai:password@evil.example/login",
        ] {
            assert!(!allowed_login_url(url, CLAUDE_DOMAINS));
        }
    }

    #[test]
    fn validates_confirmation_code_without_logging_it() {
        assert_eq!(validate_login_code("  abc123  ").unwrap(), "abc123");
        assert!(validate_login_code(&"x".repeat(513)).is_err());
        assert!(validate_login_code("abc\n123").is_err());
        assert!(validate_login_code("  \t ").is_err());
    }
}
