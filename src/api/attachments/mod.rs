mod image;
mod resolve;
mod serve;
mod upload;
#[cfg(test)]
mod tests;

pub(crate) use image::encode_base64;
pub(crate) use resolve::resolve_message_attachments;
pub(crate) use serve::serve;
pub(crate) use upload::upload;

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::http::StatusCode;
use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};
use uuid::Uuid;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

pub(crate) fn attachment_chat_dir(state: &AppState, chat_id: &str) -> ApiResult<PathBuf> {
    if Uuid::parse_str(chat_id).is_err() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    let root = crate::core::paths::data_dir(&state.config_path).map_err(internal_error)?;
    Ok(root.join("attachments").join(chat_id))
}

fn safe_file(path: &FsPath, parent: &FsPath) -> Option<PathBuf> {
    let canonical_parent = fs::canonicalize(parent).ok()?;
    let canonical = fs::canonicalize(path).ok()?;
    (canonical.starts_with(&canonical_parent) && canonical.is_file()).then_some(canonical)
}

fn internal_error(_: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Không thể xử lý ảnh đính kèm",
    )
}
