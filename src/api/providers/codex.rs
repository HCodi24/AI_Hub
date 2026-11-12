//! Codex authentication stays backend-owned; responses never include credential data.
use super::connection::{connection, ConnectionTest};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
    providers::{
        cli_support,
        codex::{home, transport::Client},
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub(crate) struct CodexConnection {
    client: Client,
    login_id: Option<String>,
    url: Option<String>,
    started: Instant,
}

pub(crate) async fn reap_connections(state: &AppState) {
    let mut sessions = state.codex_connections.lock().await;
    let expired = sessions
        .iter()
        .filter(|(_, s)| s.started.elapsed() >= Duration::from_secs(600))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for id in expired {
        if let Some(mut session) = sessions.remove(&id) {
            session.client.close().await;
        }
    }
}

#[derive(Serialize)]
pub(crate) struct Status {
    connected: bool,
    pending: bool,
    url: Option<String>,
    message: String,
    project_skills: bool,
    project_instructions: bool,
    /// Windows sandbox of this MutiS Codex home: `ready`, `notConfigured`, `updateRequired` or `unknown`.
    sandbox: String,
}

/// Windows needs a one-time sandbox setup per Codex home before Codex may write files; elsewhere it is always ready.
pub(crate) async fn sandbox_status(client: &mut Client) -> String {
    if !cfg!(windows) {
        return "ready".into();
    }
    match client.call("windowsSandbox/readiness", json!({})).await {
        Ok(value) => value["status"].as_str().unwrap_or("unknown").to_owned(),
        Err(_) => "unknown".into(),
    }
}

/// Runs the elevated sandbox setup. Windows shows its administrator prompt; the user has a few minutes to accept.
async fn setup_sandbox(client: &mut Client) -> anyhow::Result<()> {
    let started = client.call("windowsSandbox/setupStart", json!({"mode":"elevated"})).await?;
    anyhow::ensure!(started["started"].as_bool().unwrap_or(false), "Codex không bắt đầu thiết lập sandbox");
    tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            let message = client.next().await?;
            if message["method"] == "windowsSandbox/setupCompleted" {
                if message["params"]["success"].as_bool().unwrap_or(false) {
                    return Ok(());
                }
                anyhow::bail!("Thiết lập sandbox thất bại: {}", message["params"]["error"].as_str().unwrap_or("không rõ lý do"));
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Hết 4 phút chờ thiết lập sandbox (đã bấm Có ở hộp thoại quyền quản trị chưa?)"))?
}

async fn provider(state: &AppState, id: &str) -> ApiResult<ProviderConfig> {
    state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id && p.kind == "codex_cli")
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn Codex"))
}
fn error(error: anyhow::Error) -> ApiError {
    ApiError::new(StatusCode::BAD_GATEWAY, error.to_string())
}

async fn new_client(state: &AppState, provider: &ProviderConfig) -> ApiResult<Client> {
    let executable = cli_support::resolve_executable("codex", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Codex CLI trên máy"))?;
    let dir = home(&state.config_path, provider).map_err(error)?;
    Client::start(&executable, &dir, &dir).await.map_err(error)
}

fn connected(account: &Value) -> bool {
    matches!(
        account["account"]["type"].as_str(),
        Some("chatgpt" | "apiKey")
    )
}

pub(crate) async fn codex_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Status>> {
    let provider = provider(&state, &id).await?;
    let mut connections = state.codex_connections.lock().await;
    if let Some(session) = connections.get(&id) {
        if session.started.elapsed() > Duration::from_secs(600) {
            if let Some(mut session) = connections.remove(&id) {
                session.client.close().await;
            }
        }
    }
    let mut temporary;
    let (account, sandbox) = if let Some(session) = connections.get_mut(&id) {
        let account = session
            .client
            .call("account/read", json!({"refreshToken":false}))
            .await
            .map_err(error)?;
        let sandbox = sandbox_status(&mut session.client).await;
        (account, sandbox)
    } else {
        // No credentials: avoid starting a process just to show the signed-out state.
        let dir = home(&state.config_path, &provider).map_err(error)?;
        if !dir.join("auth.json").is_file() {
            let config = state.config.read().await;
            return Ok(Json(Status {
                connected: false,
                pending: false,
                url: None,
                message: "Chưa đăng nhập Codex trong MutiS".into(),
                project_skills: config.codex.project_skills,
                project_instructions: config.codex.project_instructions,
                sandbox: "unknown".into(),
            }));
        }
        temporary = new_client(&state, &provider).await?;
        let result = temporary
            .call("account/read", json!({"refreshToken":false}))
            .await;
        let sandbox = sandbox_status(&mut temporary).await;
        temporary.close().await;
        (result.map_err(error)?, sandbox)
    };
    let is_connected = connected(&account);
    let failed = connections
        .get_mut(&id)
        .is_some_and(|s| s.client.login_failed());
    if is_connected || failed {
        if let Some(mut session) = connections.remove(&id) {
            session.client.close().await;
        }
    }
    let pending = connections.get(&id).is_some_and(|s| s.login_id.is_some());
    let url = connections.get(&id).and_then(|s| s.url.clone());
    let message = if is_connected {
        "Codex đã kết nối trong MutiS"
    } else if pending {
        "Đang chờ đăng nhập trên trình duyệt"
    } else {
        "Chưa đăng nhập Codex trong MutiS"
    }
    .to_owned();
    let config = state.config.read().await;
    let status = Status {
        connected: is_connected,
        pending,
        url,
        message: message.clone(),
        project_skills: config.codex.project_skills,
        project_instructions: config.codex.project_instructions,
        sandbox,
    };
    drop(config);
    state.provider_status.lock().await.insert(
        id,
        (
            Instant::now(),
            connection(
                is_connected,
                if is_connected {
                    "connected"
                } else {
                    "not_configured"
                },
                &message,
                "",
            ),
        ),
    );
    Ok(Json(status))
}

#[derive(Deserialize)]
pub(crate) struct Action {
    action: String,
}

pub(crate) async fn codex_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Action>,
) -> ApiResult<Json<Status>> {
    let mut provider = provider(&state, &id).await?;
    if !matches!(input.action.as_str(), "cli" | "web" | "cancel" | "logout" | "sandbox") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thao tác Codex không hợp lệ",
        ));
    }
    if state.active_jobs.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Chờ AI hoàn tất trước khi thay đổi kết nối Codex",
        ));
    }
    if input.action == "sandbox" {
        // Outside the connections lock: waiting for the administrator prompt can take minutes.
        let mut client = new_client(&state, &provider).await?;
        let outcome = setup_sandbox(&mut client).await;
        client.close().await;
        state.provider_status.lock().await.remove(&id);
        outcome.map_err(error)?;
        return codex_status(State(state), Path(id)).await;
    }
    let mut connections = state.codex_connections.lock().await;
    if let Some(mut session) = connections.remove(&id) {
        if let Some(login) = session.login_id.take() {
            let _ = session
                .client
                .call("account/login/cancel", json!({"loginId":login}))
                .await;
        }
        session.client.close().await;
    }
    match input.action.as_str() {
        "cli" => {
            let executable = cli_support::resolve_executable("codex", Some(&provider.executable))
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::NOT_FOUND,
                        "Không tìm thấy Codex; hãy cài Codex CLI hoặc chọn đường dẫn chương trình",
                    )
                })?;
            provider.executable = executable.to_string_lossy().into_owned();
            let dir = home(&state.config_path, &provider).map_err(error)?;
            crate::providers::codex::environment::import_cli_auth(&dir).map_err(error)?;
            let mut config = state.config.write().await;
            if let Some(p) = config.providers.iter_mut().find(|p| p.id == id) {
                p.executable = provider.executable.clone();
            }
            config.codex.revision = config.codex.revision.wrapping_add(1);
            crate::config::save(&state.config_path, &config).map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Không lưu được kết nối Codex",
                )
            })?;
        }
        "web" => {
            let mut client = new_client(&state, &provider).await?;
            client
                .call("account/logout", json!({}))
                .await
                .map_err(error)?;
            // Ensure subsequent conversations do not resume an earlier account's context.
            let mut config = state.config.write().await;
            config.codex.revision = config.codex.revision.wrapping_add(1);
            crate::config::save(&state.config_path, &config).map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Không lưu được cấu hình Codex",
                )
            })?;
            drop(config);
            let result = client
                .call("account/login/start", json!({"type":"chatgpt"}))
                .await
                .map_err(error)?;
            let url = result["authUrl"]
                .as_str()
                .filter(|url| allowed_url(url))
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::BAD_GATEWAY,
                        "Codex trả địa chỉ đăng nhập không hợp lệ",
                    )
                })?
                .to_owned();
            let login_id = result["loginId"].as_str().map(str::to_owned);
            connections.insert(
                id.clone(),
                CodexConnection {
                    client,
                    login_id,
                    url: Some(url),
                    started: Instant::now(),
                },
            );
        }
        "logout" => {
            let mut client = new_client(&state, &provider).await?;
            let result = client.call("account/logout", json!({})).await;
            client.close().await;
            result.map_err(error)?;
            let mut config = state.config.write().await;
            config.codex.revision = config.codex.revision.wrapping_add(1);
            crate::config::save(&state.config_path, &config).map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Không lưu được cấu hình Codex",
                )
            })?;
        }
        _ => {}
    }
    drop(connections);
    state.provider_status.lock().await.remove(&id);
    crate::providers::limits::invalidate(&id);
    codex_status(State(state), Path(id)).await
}

pub(crate) async fn codex_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<crate::config::CodexConfig>,
) -> ApiResult<Json<Status>> {
    provider(&state, &id).await?;
    if state.active_jobs.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Chờ AI hoàn tất trước khi đổi nguồn hướng dẫn",
        ));
    }
    let mut config = state.config.write().await;
    config.codex.project_skills = input.project_skills;
    config.codex.project_instructions = input.project_instructions;
    config.codex.revision = config.codex.revision.wrapping_add(1);
    crate::config::save(&state.config_path, &config).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không lưu được cài đặt Codex",
        )
    })?;
    drop(config);
    codex_status(State(state), Path(id)).await
}

pub(super) async fn test_connection(state: &AppState, id: &str) -> ConnectionTest {
    match codex_status(State(state.clone()), Path(id.to_owned())).await {
        Ok(Json(status)) => connection(
            status.connected,
            if status.connected {
                "connected"
            } else {
                "not_configured"
            },
            &status.message,
            "",
        ),
        Err(_) => connection(
            false,
            "error",
            "Không kiểm tra được Codex trong MutiS",
            "Kiểm tra bản cài và đăng nhập",
        ),
    }
}

fn allowed_url(value: &str) -> bool {
    reqwest::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(
                url.host_str(),
                Some("auth.openai.com" | "auth0.openai.com" | "chatgpt.com")
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_urls_are_official_https_only() {
        assert!(allowed_url("https://auth.openai.com/oauth/authorize?a=1"));
        for url in [
            "http://auth.openai.com",
            "https://auth.openai.com.evil.test",
            "javascript:alert(1)",
            "https://user@chatgpt.com",
        ] {
            assert!(!allowed_url(url));
        }
    }

    #[tokio::test]
    async fn source_switches_persist_independently_without_authentication() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/codex-api-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.json");
        let config = crate::config::Config::seeded();
        crate::config::save(&path, &config).unwrap();
        let state = AppState::new(config, path.clone(), root.join("chats"), None, 0);
        let Json(initial) = codex_status(State(state.clone()), Path("codex-cli".into()))
            .await
            .unwrap();
        assert!(
            !initial.connected
                && !initial.pending
                && !initial.project_skills
                && !initial.project_instructions
        );
        let Json(updated) = codex_settings(
            State(state.clone()),
            Path("codex-cli".into()),
            Json(crate::config::CodexConfig {
                project_skills: true,
                project_instructions: false,
                revision: 900,
            }),
        )
        .await
        .unwrap();
        assert!(updated.project_skills && !updated.project_instructions);
        let stored: crate::config::Config =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(stored.codex.revision, 1);
        assert!(stored.codex.project_skills && !stored.codex.project_instructions);
        assert!(codex_action(
            State(state),
            Path("codex-cli".into()),
            Json(Action {
                action: "unexpected".into()
            })
        )
        .await
        .is_err());
    }
}
