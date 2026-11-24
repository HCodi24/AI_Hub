use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRequest},
};
use axum::http::StatusCode;
use std::path::PathBuf;

/// Working folder the AI runs in: dev-repo aware, must exist, returned canonical.
pub(in crate::api::chat) fn resolve_folder(
    state: &AppState,
    request: &ChatRequest,
    dev_chat: bool,
) -> ApiResult<String> {
    if !dev_chat && request.folder.trim().is_empty() {
        return plain_chat_folder(&state.config_path, &request.chat_id);
    }
    let requested_folder = if dev_chat && state.dev_repo.is_some() {
        PathBuf::from(crate::api::dev::effective_chat_folder(
            true,
            true,
            request.folder.trim(),
            &crate::api::dev::dev_repo_path(state)?.to_string_lossy(),
        ))
    } else {
        PathBuf::from(crate::api::dev::effective_chat_folder(
            false,
            dev_chat,
            request.folder.trim(),
            "",
        ))
    };
    if requested_folder.as_os_str().is_empty() || !requested_folder.is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = requested_folder
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Không thể giải quyết thư mục làm việc",
            )
        })?
        .to_string_lossy()
        .into_owned();
    Ok(folder)
}

// CLI sources need a working directory even when a chat has no project.
pub(super) fn plain_chat_folder(config: &std::path::Path, chat_id: &str) -> ApiResult<String> {
    if !chat::valid_id(chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    let folder = crate::core::paths::data_dir(config)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không tìm thấy vùng dữ liệu ứng dụng",
            )
        })?
        .join("plain-chats")
        .join(chat_id);
    std::fs::create_dir_all(&folder).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không tạo được vùng dữ liệu cho chat",
        )
    })?;
    Ok(folder.to_string_lossy().into_owned())
}
