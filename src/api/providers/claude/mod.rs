mod auth;
#[cfg(test)]
mod tests;

use self::auth::{auth_status, import_cli_account};
use super::shared::find_provider;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{ClaudeConfig, Config},
    providers::{claude::environment, cli_support},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
pub(crate) struct Status {
    connected: bool,
    pending: bool,
    message: String,
    project_skills: bool,
    project_instructions: bool,
    /// Signed-in account of this source; `origin` is `cli` (copied from this machine) or `web`.
    email: Option<String>,
    plan: Option<String>,
    origin: Option<String>,
}

pub(super) fn error(error: anyhow::Error) -> ApiError {
    eprintln!("Claude error: {error:#}");
    ApiError::new(
        StatusCode::BAD_GATEWAY,
        "Không xử lý được cấu hình hoặc đăng nhập Claude; kiểm tra bản cài và thử đăng nhập qua web",
    )
}

pub(super) use super::shared::ensure_login_mode;

pub(super) async fn ensure_idle(state: &AppState) -> ApiResult<()> {
    super::shared::ensure_idle(
        state,
        "Chờ AI hoàn tất trước khi đổi kết nối hoặc nguồn hướng dẫn Claude",
    )
}

async fn ensure_no_pending_login(state: &AppState, message: &'static str) -> ApiResult<()> {
    if state.claude_login.lock().await.is_some() {
        return Err(ApiError::new(StatusCode::CONFLICT, message));
    }
    Ok(())
}

fn update_config(
    state: &AppState,
    config: &mut Config,
    update: impl FnOnce(&mut ClaudeConfig),
) -> ApiResult<()> {
    let mut next = config.clone();
    update(&mut next.claude);
    next.claude.revision = next.claude.revision.wrapping_add(1);
    super::crud::persist_config(state, &next)?;
    *config = next;
    Ok(())
}

pub(super) async fn invalidate(state: &AppState) -> ApiResult<()> {
    let mut config = state.config.write().await;
    update_config(state, &mut config, |_| {})?;
    state.provider_status.lock().await.clear();
    Ok(())
}

pub(crate) async fn claude_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Status>> {
    let p = find_provider(&state, &id, Some("claude_cli"), "Không tìm thấy nguồn Claude").await?;
    let home = environment::home(&state.config_path, &p).map_err(error)?;
    let executable = cli_support::resolve_executable("claude", Some(&p.executable));
    let account = if let Some(executable) = executable.as_ref() {
        auth_status(executable, &home).await.map_err(error)?
    } else {
        cli_support::AuthAccount::default()
    };
    let connected = account.logged_in;
    let pending = state
        .claude_login
        .lock()
        .await
        .as_ref()
        .is_some_and(|s| s.provider_id == id);
    let origin = connected
        .then(|| cli_support::auth_origin(&home))
        .flatten();
    let message = if executable.is_none() {
        "Chưa tìm thấy Claude CLI trên máy".to_owned()
    } else if connected {
        cli_support::connected_message(account.email.as_deref(), origin.as_deref())
    } else if pending {
        "Đang chờ đăng nhập Claude trên trình duyệt".to_owned()
    } else {
        "Claude của MutiS chưa đăng nhập".to_owned()
    };
    let config = state.config.read().await;
    Ok(Json(Status {
        connected,
        pending,
        message,
        project_skills: config.claude.project_skills,
        project_instructions: config.claude.project_instructions,
        email: account.email.filter(|_| connected),
        plan: account.plan.filter(|_| connected),
        origin,
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
    let p = find_provider(&state, &id, Some("claude_cli"), "Không tìm thấy nguồn Claude").await?;
    ensure_idle(&state).await?;
    ensure_no_pending_login(&state, "Hủy hoặc hoàn tất đăng nhập Claude đang chờ trước").await?;
    let home = environment::home(&state.config_path, &p).map_err(error)?;
    match input.action.as_str() {
        "cli" => {
            ensure_login_mode(&p, false)?;
            let executable = cli_support::resolve_executable("claude", Some(&p.executable))
                .ok_or_else(|| {
                    ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Claude CLI trên máy")
                })?;
            import_cli_account(&executable, &home).await.map_err(error)?;
        }
        "logout" => {
            // Local sign-out must not revoke tokens used by the user's original CLI.
            let credentials = home.join(".credentials.json");
            if credentials.exists() {
                std::fs::remove_file(credentials).map_err(|e| error(e.into()))?;
            }
            cli_support::set_auth_origin(&home, None);
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
    find_provider(&state, &id, Some("claude_cli"), "Không tìm thấy nguồn Claude").await?;
    ensure_idle(&state).await?;
    ensure_no_pending_login(&state, "Hoàn tất hoặc hủy đăng nhập Claude trước").await?;
    let mut config = state.config.write().await;
    update_config(&state, &mut config, |claude| {
        claude.project_skills = input.project_skills;
        claude.project_instructions = input.project_instructions;
    })?;
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
