use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::ChatAttachment,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path as FsPath, PathBuf},
};
use uuid::Uuid;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

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
    let expected_mime = match ext {
        "png" => "image/png",
        "jpg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return Err(ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy ảnh")),
    };
    let path = attachment_chat_dir(&state, &chat_id.to_string())?.join(format!("{id}.{ext}"));
    let safe = safe_file(&path, &attachment_chat_dir(&state, &chat_id.to_string())?)
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
        let ext = match item.mime.as_str() {
            "image/png" => "png",
            "image/jpeg" => "jpg",
            "image/webp" => "webp",
            "image/gif" => "gif",
            _ => {
                return Err(ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "Ảnh đính kèm không hợp lệ",
                ))
            }
        };
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

fn image_type(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(("image/png", "png"))
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(("image/jpeg", "jpg"))
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(("image/webp", "webp"))
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(("image/gif", "gif"))
    } else {
        None
    }
}

pub(crate) fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        result.push(TABLE[((n >> 18) & 63) as usize] as char);
        result.push(TABLE[((n >> 12) & 63) as usize] as char);
        result.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        result.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    result
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    if input.len() > MAX_IMAGE_BYTES * 4 / 3 + 8 || !input.len().is_multiple_of(4) {
        return None;
    }
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    for chunk in input.as_bytes().chunks_exact(4) {
        let mut n = 0u32;
        let mut padding = 0;
        for (index, byte) in chunk.iter().enumerate() {
            n <<= 6;
            match byte {
                b'A'..=b'Z' => n |= u32::from(byte - b'A'),
                b'a'..=b'z' => n |= u32::from(byte - b'a' + 26),
                b'0'..=b'9' => n |= u32::from(byte - b'0' + 52),
                b'+' => n |= 62,
                b'/' => n |= 63,
                b'=' if index >= 2 => padding += 1,
                _ => return None,
            }
        }
        if padding > 2 {
            return None;
        }
        out.push((n >> 16) as u8);
        if padding < 2 {
            out.push((n >> 8) as u8);
        }
        if padding == 0 {
            out.push(n as u8);
        }
        if out.len() > MAX_IMAGE_BYTES {
            return None;
        }
    }
    Some(out)
}

fn internal_error(_: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Không thể xử lý ảnh đính kèm",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_image_magic_bytes() {
        for (bytes, mime) in [
            (b"\x89PNG\r\n\x1a\nxx".as_slice(), "image/png"),
            (&[0xff, 0xd8, 0xff, 1], "image/jpeg"),
            (b"RIFFxxxxWEBP".as_slice(), "image/webp"),
            (b"GIF89a".as_slice(), "image/gif"),
        ] {
            assert_eq!(image_type(bytes).map(|v| v.0), Some(mime));
        }
        assert!(image_type(b"not a png").is_none());
    }
    #[test]
    fn base64_roundtrip() {
        let bytes = b"image bytes";
        assert_eq!(
            decode_base64(&encode_base64(bytes)).as_deref(),
            Some(bytes.as_slice())
        );
    }
    #[test]
    fn serves_only_files_inside_chat_dir() {
        let temp = std::env::temp_dir().join(format!("ai-hub-attachments-{}", Uuid::new_v4()));
        let dir = temp.join("chat");
        fs::create_dir_all(&dir).unwrap();
        let outside = temp.join("outside.png");
        fs::write(&outside, b"x").unwrap();
        let link = dir.join("link.png");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_file(&outside, &link).unwrap();
        assert!(safe_file(&link, &dir).is_none());
        let _ = fs::remove_dir_all(temp);
    }
}
