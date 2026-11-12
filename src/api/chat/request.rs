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

// CLI sources need a working directory even when a chat has no project.
fn plain_chat_folder(config: &std::path::Path, chat_id: &str) -> ApiResult<String> {
    if !chat::valid_id(chat_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "chat_id phải là UUID hợp lệ"));
    }
    let folder = crate::core::paths::data_dir(config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không tìm thấy vùng dữ liệu ứng dụng"))?
        .join("plain-chats").join(chat_id);
    std::fs::create_dir_all(&folder).map_err(|_| ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR, "Không tạo được vùng dữ liệu cho chat",
    ))?;
    Ok(folder.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_chats_use_separate_stable_runtime_folders_and_keep_empty_project() {
        let root = std::env::temp_dir().join(format!("ai-hub-plain-{}", uuid::Uuid::new_v4()));
        let config = root.join("config.json");
        let id = uuid::Uuid::new_v4().to_string();
        let folder = plain_chat_folder(&config, &id).unwrap();
        assert!(std::path::Path::new(&folder).is_dir());
        assert_eq!(folder, plain_chat_folder(&config, &id).unwrap());
        assert_ne!(folder, plain_chat_folder(&config, &uuid::Uuid::new_v4().to_string()).unwrap());
        assert!(plain_chat_folder(&config, "../project").is_err());
        let request: ChatRequest = serde_json::from_value(serde_json::json!({
            "chat_id": id, "provider_id": "p", "model": "m", "folder": "",
            "messages": [{ "role": "user", "content": "Hello" }]
        })).unwrap();
        assert!(check_request(&request).is_ok());
        let store = chat::ChatStore::new(root.join("chats"));
        store.save(&chat::new_record(&request)).unwrap();
        assert_eq!(store.get(&id).unwrap().unwrap().folder, "");
        assert_eq!(store.list().unwrap()[0].folder, "");
        std::fs::remove_dir_all(root).unwrap();
    }
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
