//! Codex authentication stays backend-owned; responses never include credential data.

mod action;
mod sandbox;
mod session;
mod status;
#[cfg(test)]
mod tests;

pub(crate) use action::{codex_action, codex_settings};
pub(crate) use sandbox::sandbox_status;
pub(super) use session::close_connection;
pub(crate) use session::{reap_connections, CodexConnection};
pub(crate) use status::codex_status;
pub(super) use status::test_connection;

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
    providers::{
        cli_support,
        codex::{home, transport::Client},
    },
};
use axum::http::StatusCode;

#[cfg(test)]
use self::action::{allowed_url, Action};
#[cfg(test)]
use axum::{
    extract::{Path, State},
    Json,
};

fn error(error: anyhow::Error) -> ApiError {
    ApiError::new(StatusCode::BAD_GATEWAY, error.to_string())
}

pub(super) async fn new_client(state: &AppState, provider: &ProviderConfig) -> ApiResult<Client> {
    let executable = cli_support::resolve_executable("codex", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Codex CLI trên máy"))?;
    let dir = home(&state.config_path, provider).map_err(error)?;
    Client::start(&executable, &dir, &dir).await.map_err(error)
}
