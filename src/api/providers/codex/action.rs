use super::super::shared::{ensure_idle, ensure_login_mode, find_provider};
use super::{
    error, new_client,
    sandbox::setup_sandbox,
    status::{codex_status, Status},
    CodexConnection,
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::Config,
    providers::{cli_support, codex::home},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::time::Instant;

#[derive(Deserialize)]
pub(crate) struct Action {
    pub(super) action: String,
}

pub(crate) async fn codex_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<Action>,
) -> ApiResult<Json<Status>> {
    let mut provider =
        find_provider(&state, &id, Some("codex_cli"), "Không tìm thấy nguồn Codex").await?;
    if !matches!(
        input.action.as_str(),
        "cli" | "web" | "cancel" | "logout" | "sandbox"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thao tác Codex không hợp lệ",
        ));
    }
    match input.action.as_str() {
        "cli" => ensure_login_mode(&provider, false)?,
        "web" => ensure_login_mode(&provider, true)?,
        _ => {}
    }
    ensure_idle(&state, "Chờ AI hoàn tất trước khi thay đổi kết nối Codex")?;
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
    if let Some(session) = connections.remove(&id) {
        session.cancel_and_close().await;
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
            cli_support::set_auth_origin(&dir, Some("cli"));
            update_config(&state, "Không lưu được kết nối Codex", |config| {
                if let Some(p) = config.providers.iter_mut().find(|p| p.id == id) {
                    p.executable = provider.executable.clone();
                }
            })
            .await?;
        }
        "web" => {
            // Drop the previous account locally (never revoke: it may be shared with the machine's Codex),
            // then start Codex without credentials so it begins a fresh sign-in.
            let dir = home(&state.config_path, &provider).map_err(error)?;
            remove_local_auth(&dir).map_err(error)?;
            // The previous account is gone; this marker only counts once the web sign-in connects.
            cli_support::set_auth_origin(&dir, Some("web"));
            let mut client = new_client(&state, &provider).await?;
            // Ensure subsequent conversations do not resume an earlier account's context.
            update_config(&state, "Không lưu được cấu hình Codex", |_| {}).await?;
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
            // Local sign-out only: `account/logout` revokes the tokens server-side, and a source copied
            // from this machine shares them with the user's own Codex/ChatGPT, which would be signed out too.
            let dir = home(&state.config_path, &provider).map_err(error)?;
            remove_local_auth(&dir).map_err(error)?;
            cli_support::set_auth_origin(&dir, None);
            update_config(&state, "Không lưu được cấu hình Codex", |_| {}).await?;
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
    find_provider(&state, &id, Some("codex_cli"), "Không tìm thấy nguồn Codex").await?;
    ensure_idle(&state, "Chờ AI hoàn tất trước khi đổi nguồn hướng dẫn")?;
    update_config(&state, "Không lưu được cài đặt Codex", |config| {
        config.codex.project_skills = input.project_skills;
        config.codex.project_instructions = input.project_instructions;
    })
    .await?;
    codex_status(State(state), Path(id)).await
}

async fn update_config(
    state: &AppState,
    message: &'static str,
    update: impl FnOnce(&mut Config),
) -> ApiResult<()> {
    let mut config = state.config.write().await;
    update(&mut config);
    config.codex.revision = config.codex.revision.wrapping_add(1);
    crate::config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message))
}

/// Deletes only this source's credential file; the tokens stay valid wherever else they are used.
fn remove_local_auth(dir: &std::path::Path) -> anyhow::Result<()> {
    match std::fs::remove_file(dir.join("auth.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Không xoá được đăng nhập Codex của nguồn này: {e}")),
    }
}

pub(super) fn allowed_url(value: &str) -> bool {
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
