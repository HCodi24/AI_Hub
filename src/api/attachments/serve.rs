use super::{
    attachment_chat_dir,
    image::{image_type, mime_for_ext},
    internal_error, safe_file,
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use std::fs;
use uuid::Uuid;

pub(crate) async fn serve(
    State(state): State<AppState>,
    Path((chat, file)): Path<(String, String)>,
) -> ApiResult<Response> {
    let chat_id = Uuid::parse_str(&chat)
        .map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"))?;
    let (id, ext) = file
        .rsplit_once('.')
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"))?;
    Uuid::parse_str(id).map_err(|_| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"))?;
    let expected_mime = mime_for_ext(ext)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"))?;
    let dir = attachment_chat_dir(&state, &chat_id.to_string())?;
    let path = dir.join(format!("{id}.{ext}"));
    let safe = safe_file(&path, &dir)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"))?;
    let bytes = fs::read(safe).map_err(internal_error)?;
    if image_type(&bytes).is_none_or(|(mime, _)| mime != expected_mime) {
        return Err(ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh"));
    }
    let mut response = bytes.into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(expected_mime),
    );
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}
