//! Validation of an incoming chat request.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRequest, Effort},
};
use axum::http::StatusCode;
use std::path::PathBuf;

/// Working folder the AI runs in: dev-repo aware, must exist, returned canonical.
pub(super) fn resolve_folder(state: &AppState, request: &ChatRequest, dev_chat: bool) -> ApiResult<String> {
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
    let folder = requested_folder.to_string_lossy().into_owned();
    if folder.is_empty() || !PathBuf::from(&folder).is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = PathBuf::from(folder)
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

/// Model, effort and message-history checks; returns the parsed effort.
pub(super) fn check_request(request: &ChatRequest) -> ApiResult<Effort> {
    if request.model.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Hãy chọn model"));
    }
    let effort = request.effort.as_deref().unwrap_or("default");
    let effort = chat::parse_effort(effort)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Mức suy luận không hợp lệ"))?;
    if request.messages.is_empty()
        || request
            .messages
            .iter()
            .any(|message| !matches!(message.role.as_str(), "user" | "assistant" | "system"))
        || request.messages.last().map(|message| message.role.as_str()) != Some("user")
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Lịch sử chat không hợp lệ",
        ));
    }
    Ok(effort)
}
