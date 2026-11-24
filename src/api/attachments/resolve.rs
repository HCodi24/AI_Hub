use super::{attachment_chat_dir, image::ext_for_mime, internal_error, safe_file, MAX_IMAGE_BYTES};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::ChatAttachment,
};
use axum::http::StatusCode;
use std::fs;
use uuid::Uuid;

pub(crate) fn resolve_message_attachments(
    state: &AppState,
    chat_id: &str,
    attachments: &[ChatAttachment],
) -> ApiResult<Vec<ChatAttachment>> {
    if attachments.len() > 5 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mỗi tin nhắn tối đa 5 ảnh",
        ));
    }
    let dir = attachment_chat_dir(state, chat_id)?;
    let canonical_dir = fs::canonicalize(&dir).unwrap_or(dir.clone());
    let mut resolved = Vec::new();
    for item in attachments {
        let id = Uuid::parse_str(&item.id)
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Ảnh đính kèm không hợp lệ"))?;
        let ext = ext_for_mime(&item.mime)
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Ảnh đính kèm không hợp lệ"))?;
        let path = dir.join(format!("{id}.{ext}"));
        let safe = safe_file(&path, &canonical_dir)
            .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Không tìm thấy ảnh đính kèm"))?;
        let metadata = fs::metadata(&safe).map_err(internal_error)?;
        if metadata.len() > MAX_IMAGE_BYTES as u64 {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Ảnh đính kèm vượt quá 10 MB",
            ));
        }
        resolved.push(ChatAttachment {
            id: id.to_string(),
            mime: item.mime.clone(),
            path: safe.to_string_lossy().into_owned(),
        });
    }
    Ok(resolved)
}
