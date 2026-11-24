//! Antigravity CLI uses the machine's sign-in. MutiS stores only its local sign-out state.

pub(super) use crate::features::providers::cli::antigravity::models::list_models;
pub(crate) use crate::features::providers::cli::antigravity::probe::test_connection;

pub(crate) use crate::features::providers::cli::antigravity::EMAIL_FILE;
use super::connection::ConnectionTest;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::providers::{
        login_cli::antigravity::{connect_machine_account, logout as logout_cli},
        login_web::antigravity::logout as logout_web,
    },
    providers::antigravity_cli::{clean_home, SIGNED_OUT_FILE},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    Cli,
    Logout,
}

pub(crate) async fn antigravity_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(action): Json<Action>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.id == id && provider.kind == "antigravity_cli")
        .cloned()
        .ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn Antigravity CLI")
        })?;
    let error = |error: &dyn std::fmt::Display| {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    };
    let home = clean_home(&id).map_err(|err| error(&err))?;
    let marker = home.join(SIGNED_OUT_FILE);
    let result = match action {
        Action::Cli => connect_machine_account(&provider, &home, &marker)
            .await
            .map_err(|err| error(&err))?,
        Action::Logout if provider.web_login => logout_web(&home).map_err(|err| error(&err))?,
        Action::Logout => logout_cli(&marker).map_err(|err| error(&err))?,
    };
    state.provider_status.lock().await.remove(&id);
    Ok(Json(result))
}
