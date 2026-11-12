use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{ClaudeConfig, ProviderConfig},
    providers::{claude::environment, cli_support},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::{path::Path as FsPath, process::Stdio, time::Duration};

#[derive(Serialize)]
pub(crate) struct Status {
    connected: bool,
    pending: bool,
    message: String,
    project_skills: bool,
    project_instructions: bool,
}

pub(super) fn error(_: anyhow::Error) -> ApiError {
    ApiError::new(StatusCode::BAD_GATEWAY, "Không xử lý được cấu hình hoặc đăng nhập Claude; kiểm tra bản cài và thử đăng nhập qua web")
}

async fn provider(state: &AppState, id: &str) -> ApiResult<ProviderConfig> {
    state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id && p.kind == "claude_cli")
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn Claude"))
}

pub(super) async fn ensure_idle(state: &AppState) -> ApiResult<()> {
    if state.active_jobs.load(std::sync::atomic::Ordering::SeqCst) > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Chờ AI hoàn tất trước khi đổi kết nối hoặc nguồn hướng dẫn Claude",
        ));
    }
    Ok(())
}

pub(super) async fn invalidate(state: &AppState) -> ApiResult<()> {
    let mut config = state.config.write().await;
    let mut next = config.clone();
    next.claude.revision = next.claude.revision.wrapping_add(1);
    super::crud::persist_config(state, &next)?;
    *config = next;
    state.provider_status.lock().await.clear();
    Ok(())
}

pub(super) async fn auth_status(executable: &FsPath, home: &FsPath) -> anyhow::Result<bool> {
    // Never mistake a machine-wide/keychain login for this source's file credentials.
    if !home.join(".credentials.json").is_file() {
        return Ok(false);
    }
    let mut command = cli_support::command_for(executable, &["auth".into(), "status".into()]);
    environment::apply(&mut command, home);
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        command
            .current_dir(home)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    cli_support::parse_auth_logged_in(&output.stdout)
        .ok_or_else(|| anyhow::anyhow!("Claude auth status không trả JSON hợp lệ"))
}

pub(crate) async fn claude_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Status>> {
    let p = provider(&state, &id).await?;
    let home = environment::home(&state.config_path, &p).map_err(error)?;
    let executable = cli_support::resolve_executable("claude", Some(&p.executable));
    let connected = if let Some(executable) = executable.as_ref() {
        auth_status(executable, &home).await.map_err(error)?
    } else {
        false
    };
    let pending = state
        .claude_login
        .lock()
        .await
        .as_ref()
        .is_some_and(|s| s.provider_id == id);
    let message = if executable.is_none() {
        "Chưa tìm thấy Claude CLI trên máy"
    } else if connected {
        "Claude đã kết nối trong MutiS"
    } else if pending {
        "Đang chờ đăng nhập Claude trên trình duyệt"
    } else {
        "Claude của MutiS chưa đăng nhập"
    };
    let config = state.config.read().await;
    Ok(Json(Status {
        connected,
        pending,
        message: message.into(),
        project_skills: config.claude.project_skills,
        project_instructions: config.claude.project_instructions,
    }))
}

#[derive(Deserialize)]
pub(crate) struct Action {
    action: String,
}

pub(crate) async fn claude_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Action>,
) -> ApiResult<Json<Status>> {
    let p = provider(&state, &id).await?;
    ensure_idle(&state).await?;
    if state.claude_login.lock().await.is_some() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Hủy hoặc hoàn tất đăng nhập Claude đang chờ trước",
        ));
    }
    let home = environment::home(&state.config_path, &p).map_err(error)?;
    match input.action.as_str() {
        "cli" => {
            let executable = cli_support::resolve_executable("claude", Some(&p.executable))
                .ok_or_else(|| {
                    ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Claude CLI trên máy")
                })?;
            // Validate the allow-listed credentials in a temporary app-owned directory first.
            let stage = home.join(format!(".auth-check-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&stage).map_err(|e| error(e.into()))?;
            let result = async {
                environment::import_cli_auth(&stage)?;
                if !auth_status(&executable, &stage).await? {
                    anyhow::bail!("CLI chưa đăng nhập");
                }
                environment::import_auth_file(&stage.join(".credentials.json"), &home)?;
                Ok::<_, anyhow::Error>(())
            }
            .await;
            // Only remove files created by this operation, never recursively delete a computed path.
            let _ = std::fs::remove_file(stage.join(".credentials.json"));
            let _ = std::fs::remove_file(stage.join(".claude.json"));
            let _ = std::fs::remove_dir(&stage);
            result.map_err(error)?;
        }
        "logout" => {
            // Local sign-out must not revoke tokens used by the user's original CLI.
            let credentials = home.join(".credentials.json");
            if credentials.exists() {
                std::fs::remove_file(credentials).map_err(|e| error(e.into()))?;
            }
        }
        _ => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Thao tác Claude không hợp lệ",
            ))
        }
    }
    invalidate(&state).await?;
    crate::providers::limits::invalidate(&id);
    claude_status(State(state), Path(id)).await
}

pub(crate) async fn claude_settings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ClaudeConfig>,
) -> ApiResult<Json<Status>> {
    provider(&state, &id).await?;
    ensure_idle(&state).await?;
    if state.claude_login.lock().await.is_some() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Hoàn tất hoặc hủy đăng nhập Claude trước",
        ));
    }
    let mut config = state.config.write().await;
    let mut next = config.clone();
    next.claude.project_skills = input.project_skills;
    next.claude.project_instructions = input.project_instructions;
    next.claude.revision = next.claude.revision.wrapping_add(1);
    super::crud::persist_config(&state, &next)?;
    *config = next;
    drop(config);
    claude_status(State(state), Path(id)).await
}

pub(super) async fn test_connection(state: &AppState, id: &str) -> super::ConnectionTest {
    match claude_status(State(state.clone()), Path(id.to_owned())).await {
        Ok(Json(status)) => super::connection::connection(
            status.connected,
            if status.connected {
                "connected"
            } else {
                "not_configured"
            },
            &status.message,
            "",
        ),
        Err(_) => super::connection::connection(
            false,
            "error",
            "Không kiểm tra được đăng nhập Claude trong MutiS",
            "Kiểm tra bản cài và đăng nhập lại",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn claude_settings_persist_without_credentials_and_reject_busy_changes() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/claude-api-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("config.json");
        let config = crate::config::Config::seeded();
        crate::config::save(&path, &config).unwrap();
        let state = AppState::new(config, path.clone(), root.join("chats"), None, 7788);
        let id = "claude-cli".to_owned();
        let response = claude_settings(
            State(state.clone()),
            Path(id.clone()),
            Json(ClaudeConfig {
                project_skills: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(response.project_skills && !response.project_instructions && !response.connected);
        let response = claude_settings(
            State(state.clone()),
            Path(id.clone()),
            Json(ClaudeConfig {
                project_instructions: true,
                ..Default::default()
            }),
        )
        .await
        .unwrap()
        .0;
        assert!(!response.project_skills && response.project_instructions);
        let saved: crate::config::Config =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.claude.revision, 2);
        assert!(!saved.claude.project_skills && saved.claude.project_instructions);
        state
            .active_jobs
            .store(1, std::sync::atomic::Ordering::SeqCst);
        assert!(claude_settings(
            State(state.clone()),
            Path(id),
            Json(ClaudeConfig::default())
        )
        .await
        .is_err());
        assert_eq!(state.config.read().await.claude.revision, 2);
    }
}
