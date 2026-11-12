//! Dev-mode endpoints, split into focused submodules.
//!
//! Shared guards (`dev_repo_path`, `ensure_no_jobs`) live here; the HTTP
//! handlers live in the submodules and are re-exported so the old
//! `crate::api::dev::*` paths keep working.

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::http::StatusCode;
use std::{path::PathBuf, sync::atomic::Ordering};

mod build;
mod chat;
mod git;
mod settings;
mod stream;
mod watcher;

pub(crate) use build::{dev_rebuild, dev_release, dev_test, dev_web_build};
pub(crate) use chat::{dev_chat_preamble, effective_chat_folder};
pub(crate) use git::{
    dev_git_checkpoint, dev_git_commit, dev_git_diff, dev_git_reset, dev_repo_info,
};
pub(crate) use settings::{dev_settings, dev_status, put_dev_settings};
pub(crate) use stream::dev_events;
pub(crate) use watcher::spawn_dev_watcher;

pub(crate) fn dev_repo_path(state: &AppState) -> ApiResult<PathBuf> {
    state
        .dev_repo
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chế độ phát triển đang tắt"))
}

pub(crate) fn ensure_no_jobs(state: &AppState) -> ApiResult<()> {
    let jobs = state.active_jobs.load(Ordering::SeqCst);
    if jobs > 0 {
        Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Đang có {jobs} việc AI chạy — chờ xong hoặc dừng"),
        ))
    } else {
        Ok(())
    }
}
