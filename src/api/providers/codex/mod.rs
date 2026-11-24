//! Codex authentication stays backend-owned; responses never include credential data.

mod action;
mod sandbox;
mod session;
mod status;
#[cfg(test)]
mod tests;

pub(crate) use action::{codex_action, codex_settings};
pub(crate) use crate::features::providers::cli::codex::{new_client, CodexConnection};
use crate::features::providers::cli::codex::provider_error;
#[cfg(test)]
use crate::features::providers::cli::codex::allowed_url;
pub(crate) use sandbox::sandbox_status;
pub(super) use session::close_connection;
pub(crate) use session::reap_connections;
pub(crate) use status::codex_status;
pub(super) use status::test_connection;

use crate::api::ApiError;
#[cfg(test)]
use crate::app::AppState;

#[cfg(test)]
use self::action::Action;
#[cfg(test)]
use axum::{
    extract::{Path, State},
    Json,
};

pub(crate) fn error(error: anyhow::Error) -> ApiError {
    provider_error(error).into()
}
