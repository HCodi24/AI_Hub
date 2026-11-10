use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config, ProviderConfig},
    providers::{self, discovery},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ProviderInput {
    #[serde(default)]
    id: Option<String>,
    name: String,
    kind: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    executable: String,
    #[serde(default)]
    models: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    models: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct CliStatus {
    codex_cli: CliInfo,
    claude_cli: CliInfo,
}

#[derive(Serialize)]
struct CliInfo {
    installed: bool,
    version: Option<String>,
    path: Option<String>,
}

pub(crate) async fn create_provider(
    State(state): State<AppState>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_provider(&id, &input)?;
    if config.providers.iter().any(|provider| provider.id == id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Mã nhà cung cấp đã tồn tại",
        ));
    }
    let provider = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key: input.api_key.unwrap_or_default(),
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
    };
    config.providers.push(provider);
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    validate_provider(&id, &input)?;
    let mut config = state.config.write().await;
    let Some(index) = config
        .providers
        .iter()
        .position(|provider| provider.id == id)
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    };
    let old_key = config.providers[index].api_key.clone();
    let submitted_key = input.api_key.unwrap_or_default();
    let api_key = if submitted_key.trim().is_empty() || config::is_masked_key(&submitted_key) {
        old_key
    } else {
        submitted_key
    };
    config.providers[index] = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key,
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
    };
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn remove_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let previous_len = config.providers.len();
    config.providers.retain(|provider| provider.id != id);
    if config.providers.len() == previous_len {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    }
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn use_deploy_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let key = config
        .providers
        .iter()
        .find(|p| p.id == "deploy-deepseek" && p.kind == "openai_compat")
        .map(|p| p.api_key.clone())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Chưa có key trong nguồn Deploy (DeepSeek)",
            )
        })?;
    let target = config
        .providers
        .iter_mut()
        .find(|p| p.id == id && p.kind == "deepseek_agent")
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "Không tìm thấy nguồn DeepSeek làm việc",
            )
        })?;
    target.api_key = key;
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ModelsResponse>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    if !matches!(provider.kind.as_str(), "openai_compat" | "deepseek_agent") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho OpenAI-compatible",
        ));
    }
    if provider.api_key.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy lưu API key trước khi tải model",
        ));
    }
    let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Không thể kết nối tới API models"))?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("API models trả về HTTP {}", response.status()),
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Phản hồi models không hợp lệ"))?;
    let mut models = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    {
        let mut config = state.config.write().await;
        if let Some(entry) = config.providers.iter_mut().find(|entry| entry.id == id) {
            entry.models = models.clone();
            persist_config(&state, &config)?;
        }
    }
    Ok(Json(ModelsResponse { models }))
}

#[derive(Clone, Serialize)]
pub(crate) struct ConnectionTest {
    ok: bool,
    pub(crate) status: &'static str,
    message: String,
    detail: String,
}

async fn test_provider_uncached(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    let result = match provider.kind.as_str() {
        "openai_compat" | "deepseek_agent" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "openai_compat" | "deepseek_agent" => {
            let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
            match timeout(
                Duration::from_secs(15),
                reqwest::Client::new()
                    .get(url)
                    .bearer_auth(&provider.api_key)
                    .send(),
            )
            .await
            {
                Err(_) => connection(
                    false,
                    "error",
                    "Không kết nối được",
                    "Hết thời gian chờ kết nối",
                ),
                Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
                Ok(Ok(response)) if response.status().is_success() => {
                    let detail = timeout(Duration::from_secs(15), response.json::<Value>())
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or(Value::Null);
                    let count = detail["data"].as_array().map_or(0, Vec::len);
                    connection(
                        true,
                        "connected",
                        &format!("Đã kết nối · {count} model"),
                        &format!("Đã nhận {count} model từ {}", provider.base_url),
                    )
                }
                Ok(Ok(response)) => match response.status().as_u16() {
                    401 | 403 => connection(
                        false,
                        "error",
                        "Sai API key",
                        &format!("HTTP {}", response.status()),
                    ),
                    404 => connection(false, "error", "Sai Base URL", "HTTP 404"),
                    _ => connection(
                        false,
                        "error",
                        "Không kết nối được",
                        &format!("HTTP {}", response.status()),
                    ),
                },
            }
        }
        "anthropic" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "anthropic" => match timeout(
            Duration::from_secs(15),
            reqwest::Client::new()
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01")
                .send(),
        )
        .await
        {
            Err(_) => connection(
                false,
                "error",
                "Không kết nối được",
                "Hết thời gian chờ kết nối",
            ),
            Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
            Ok(Ok(r)) if r.status().is_success() => connection(
                true,
                "connected",
                "Đã kết nối",
                "Anthropic API phản hồi thành công",
            ),
            Ok(Ok(r)) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => connection(
                false,
                "error",
                "Sai API key",
                &format!("HTTP {}", r.status()),
            ),
            Ok(Ok(r)) => connection(
                false,
                "error",
                "Không kết nối được",
                &format!("HTTP {}", r.status()),
            ),
        },
        "codex_cli" | "claude_cli" => {
            let name = if provider.kind == "codex_cli" {
                "codex"
            } else {
                "claude"
            };
            match discovery::cli_probe(name, &provider.executable, Duration::from_secs(10)).await {
                Some((version, path)) if name == "claude" => {
                    match discovery::run_cli_auth_status(
                        std::path::Path::new(&path),
                        Duration::from_secs(10),
                    )
                    .await
                    {
                        Some(true) => {
                            connection(true, "connected", &format!("Đã kết nối · {version}"), &path)
                        }
                        Some(false) => {
                            connection(false, "error", "Claude Code chưa đăng nhập", &path)
                        }
                        None => {
                            connection(true, "connected", &format!("Đã kết nối · {version}"), &path)
                        }
                    }
                }
                Some((version, path)) => connection(
                    true,
                    "connected",
                    &format!("Đã kết nối · {version} · {path}"),
                    &path,
                ),
                None if providers::cli_support::resolve_executable(
                    name,
                    Some(&provider.executable),
                )
                .is_none() =>
                {
                    connection(
                        false,
                        "not_configured",
                        if name == "codex" {
                            "Chưa cài Codex CLI"
                        } else {
                            "Chưa cài Claude Code CLI"
                        },
                        if name == "codex" {
                            "npm i -g @openai/codex"
                        } else {
                            "npm i -g @anthropic-ai/claude-code"
                        },
                    )
                }
                None => connection(
                    false,
                    "error",
                    "Không kết nối được",
                    "Không chạy được --version trong 10 giây hoặc phản hồi không hợp lệ",
                ),
            }
        }
        _ => connection(
            false,
            "error",
            "Không kết nối được",
            "Loại nhà cung cấp không được hỗ trợ",
        ),
    };
    Ok(Json(result))
}

pub(crate) async fn cached_provider_test(state: &AppState, id: &str) -> ApiResult<ConnectionTest> {
    if let Some((checked, status)) = state.provider_status.lock().await.get(id).cloned() {
        if checked.elapsed() < Duration::from_secs(60) {
            return Ok(status);
        }
    }
    let status = test_provider_uncached(State(state.clone()), Path(id.to_owned()))
        .await?
        .0;
    state
        .provider_status
        .lock()
        .await
        .insert(id.to_owned(), (Instant::now(), status.clone()));
    Ok(status)
}

pub(crate) async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    if let Some(session) = state.claude_login.lock().await.as_mut() {
        if session.provider_id == id {
            session.last_activity = Instant::now();
        }
    }
    state.provider_status.lock().await.remove(&id);
    cached_provider_test(&state, &id).await.map(Json)
}

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
fn allowed_login_url(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    if scheme != "https" {
        return false;
    }
    let host = rest
        .split(['/', '?', '#', ':'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    ["claude.com", "claude.ai", "anthropic.com"]
        .iter()
        .any(|domain| host == *domain || host.ends_with(&format!(".{domain}")))
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
    if provider.kind != "claude_cli" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ Claude Code CLI mới đăng nhập được",
        ));
    }
    Ok(provider)
}

pub(crate) async fn login_provider_start(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<LoginStartResponse>> {
    let provider = login_provider_config(&state, &id).await?;
    let path = providers::cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy Claude Code CLI"))?;
    if let Some(mut old) = state.claude_login.lock().await.take() {
        let _ = old.child.kill().await;
    }
    let args = ["auth", "login", "--claudeai"].map(OsString::from);
    let mut command = providers::cli_support::command_for(&path, &args);
    providers::cli_support::clean_claude_environment(&mut command);
    #[cfg(windows)]
    command.env("BROWSER", "cmd /c rem");
    #[cfg(not(windows))]
    command.env("BROWSER", "true");
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
    if !allowed_login_url(&url) {
        let _ = session.child.start_kill();
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Claude CLI trả link đăng nhập không được phép",
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
    login_provider_config(&state, &id).await?;
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
    Ok(Json(LoginCodeResponse {
        ok: exited && logged_in,
        logged_in,
        message: if logged_in {
            "Đã đăng nhập Claude Code CLI".into()
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

fn connection(ok: bool, status: &'static str, message: &str, detail: &str) -> ConnectionTest {
    ConnectionTest {
        ok,
        status,
        message: message.into(),
        detail: detail.into(),
    }
}
pub(crate) async fn discover_cli(
    Path(name): Path<String>,
) -> ApiResult<Json<Vec<discovery::CliCandidate>>> {
    if !["codex", "claude"].contains(&name.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CLI kh\u{00f4}ng h\u{1ee3}p l\u{1ec7}",
        ));
    }
    Ok(Json(discovery::discover_candidates(&name).await))
}

pub(crate) async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
    let config = state.config.read().await;
    let codex_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
    let claude_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "claude_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
    drop(config);
    let (codex_cli, claude_cli) = tokio::join!(
        cli_info("codex", codex_executable),
        cli_info("claude", claude_executable)
    );
    Json(CliStatus {
        codex_cli,
        claude_cli,
    })
}

async fn cli_info(name: &str, explicit_path: String) -> CliInfo {
    let (installed, version, path) = discovery::probe_cli_info(name, &explicit_path).await;
    CliInfo {
        installed,
        version,
        path,
    }
}

fn validate_provider(id: &str, input: &ProviderInput) -> ApiResult<()> {
    if !valid_provider_id(id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã nhà cung cấp không hợp lệ",
        ));
    }
    if input.name.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tên nhà cung cấp không được để trống",
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "openai_compat" | "deepseek_agent" | "anthropic" | "codex_cli" | "claude_cli"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nhà cung cấp không hợp lệ",
        ));
    }
    if matches!(input.kind.as_str(), "openai_compat" | "deepseek_agent")
        && input.base_url.trim().is_empty()
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "OpenAI-compatible cần URL cơ sở",
        ));
    }
    Ok(())
}

fn valid_provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn clean_models(models: Vec<String>) -> Vec<String> {
    models
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect()
}

fn masked_config(config: &Config) -> Config {
    let mut config = config.clone();
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    config
}

fn persist_config(state: &AppState, config: &Config) -> ApiResult<()> {
    config::save(&state.config_path, config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))
}

#[cfg(test)]
mod claude_login_tests {
    use super::{allowed_login_url, extract_https_url, validate_login_code};

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
        assert!(allowed_login_url(url));
    }

    #[test]
    fn allows_only_claude_and_anthropic_hosts() {
        for host in ["claude.com", "accounts.claude.ai", "platform.anthropic.com"] {
            assert!(allowed_login_url(&format!("https://{host}/oauth")));
        }
        for url in [
            "http://claude.com/login",
            "https://evilclaude.com/login",
            "https://example.com/?next=claude.com",
        ] {
            assert!(!allowed_login_url(url));
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
