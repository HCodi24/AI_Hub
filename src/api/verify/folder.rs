use crate::api::{ApiError, ApiResult};
use axum::http::StatusCode;
use std::path::PathBuf;

pub(super) fn project_folder(raw: &str) -> ApiResult<PathBuf> {
    let folder = PathBuf::from(raw)
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Thư mục không tồn tại"))?;
    if !folder.is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Đường dẫn không phải thư mục",
        ));
    }
    Ok(folder)
}
