//! Antigravity CLI uses the machine's sign-in. MutiS stores only its local sign-out state.

mod models;
mod probe;
#[cfg(test)]
mod tests;

pub(super) use models::list_models;
pub(super) use probe::test_connection;

use self::probe::probe;
use super::connection::{connection, ConnectionTest};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    providers::antigravity_cli::{clean_home, SIGNED_OUT_FILE},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use std::{io, path::Path as FsPath};

/// The signed-in account's email, saved by the web sign-in.
/// agy prints "Logged in as <email>." and has no command that shows it later.
pub(crate) const EMAIL_FILE: &str = ".mutis-email";

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
        Action::Cli => {
            let result = probe(&provider, &home).await;
            if result.status == "connected" {
                remove_if_exists(&marker).map_err(|err| error(&err))?;
            }
            result
        }
        // Signing out deletes the web account's token file from its own home.
        // A machine source only remembers the sign-out.
        Action::Logout if provider.web_login => {
            for file in [
                home.join(".gemini")
                    .join("antigravity-cli")
                    .join("antigravity-oauth-token"),
                home.join(EMAIL_FILE),
            ] {
                remove_if_exists(&file).map_err(|err| error(&err))?;
            }
            signed_out()
        }
        Action::Logout => {
            std::fs::write(&marker, []).map_err(|err| error(&err))?;
            signed_out()
        }
    };
    state.provider_status.lock().await.remove(&id);
    Ok(Json(result))
}

fn signed_out() -> ConnectionTest {
    connection(
        false,
        "not_configured",
        "Antigravity CLI chưa đăng nhập trong MutiS",
        "",
    )
}

fn remove_if_exists(path: &FsPath) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
