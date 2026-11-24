use super::{
    attachment_chat_dir,
    image::{decode_base64, image_type},
    internal_error, MAX_IMAGE_BYTES,
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use std::fs;
use uuid::Uuid;

#[derive(Deserialize)]
pub(crate) struct UploadRequest {
    chat_id: String,
    data: String,
}

#[derive(Serialize)]
pub(crate) struct UploadResponse {
    id: String,
    url: String,
    mime: String,
    bytes: usize,
}

pub(crate) async fn upload(
    State(state): State<AppState>,
    Json(request): Json<UploadRequest>,
) -> ApiResult<Json<UploadResponse>> {
    if Uuid::parse_str(&request.chat_id).is_err() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    let bytes = decode_base64(&request.data)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Ảnh mã hóa không hợp lệ"))?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_BYTES {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Ảnh phải nhỏ hơn hoặc bằng 10 MB",
        ));
    }
    let (mime, ext) = image_type(&bytes).ok_or_else(|| {
        ApiError::new(StatusCode::BAD_REQUEST, "Chỉ hỗ trợ PNG, JPEG, WebP và GIF")
    })?;
    let id = Uuid::new_v4().to_string();
    let dir = attachment_chat_dir(&state, &request.chat_id)?;
    fs::create_dir_all(&dir).map_err(internal_error)?;
    fs::write(dir.join(format!("{id}.{ext}")), &bytes).map_err(internal_error)?;
    Ok(Json(UploadResponse {
        id: id.clone(),
        url: format!("/api/attachments/{}/{}.{}", request.chat_id, id, ext),
        mime: mime.to_owned(),
        bytes: bytes.len(),
    }))
}
