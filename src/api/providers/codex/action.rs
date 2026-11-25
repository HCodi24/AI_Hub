use super::super::shared::{ensure_idle, ensure_login_mode, find_provider};
use super::{
    error, new_client,
    sandbox::setup_sandbox,
    status::{codex_status, Status},
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::providers::{
        cli::codex::update_config,
        login_cli::codex::connect_machine_account, login_web::codex::start_web_login,
        logout::codex::logout,
    },
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;

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
        "cli" => connect_machine_account(&state, &mut provider, &id).await?,
        "web" => start_web_login(&state, &provider, &id, &mut connections).await?,
        "logout" => logout(&state, &provider).await?,
        _ => {}
    }
    drop(connections);
    state.provider_status.lock().await.remove(&id);
    crate::features::providers::limits::invalidate(&id);
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
