//! Pet sprite sheets for the chat avatars. None ship with this app: the user uploads a WebP sheet or imports a pet folder in Settings,
//! and each pet lives in `<app data>/pets/<id>/`.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};

/// Largest sprite sheet accepted.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Every sprite sheet is 8 columns of 192 px and 5 rows of 208 px (thinking, running, idle, waiting, failed).
const SHEET_WIDTH: u32 = 1536;
/// 6 rows (thinking, replying, idle, waiting, failed, running) or 5 without the running row.
const SHEET_HEIGHTS: [u32; 2] = [208 * 5, 208 * 6];

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "Không có pet này")
}

fn safe_name(name: &str) -> bool {
    (1..=48).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `<app data>/pets`: one folder per pet the user added.
fn pets_dir(state: &AppState) -> Option<PathBuf> {
    crate::core::paths::data_dir(&state.config_path).ok().map(|dir| dir.join("pets"))
}

pub(crate) async fn sprite(State(state): State<AppState>, Path(name): Path<String>) -> ApiResult<Response> {
    let bytes = installed(&state, &name).ok_or_else(not_found)?;
    Ok(image(bytes, "max-age=60"))
}

fn image(bytes: Vec<u8>, cache: &'static str) -> Response {
    let mime = if is_png(&bytes) { "image/png" } else { "image/webp" };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// Where a pet's sprite sheet is kept (the name becomes a path segment, so it is checked first).
fn downloaded_file(state: &AppState, id: &str) -> Option<PathBuf> {
    safe_name(id).then(|| pets_dir(state)).flatten().map(|dir| dir.join(id).join("spritesheet.png"))
}

/// The stored sheet of a pet, when it is still a valid one.
fn installed(state: &AppState, name: &str) -> Option<Vec<u8>> {
    let bytes = fs::read(downloaded_file(state, name)?).ok()?;
    (bytes.len() <= MAX_BYTES && is_sheet(&bytes)).then_some(bytes)
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG")
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// PNG with the pet sheet layout: 1536 px wide, 1248 px (6 rows) or 1040 px (5 rows) high.
fn is_sheet_png(bytes: &[u8]) -> bool {
    let dimension = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    is_png(bytes) && dimension(16) == Some(SHEET_WIDTH) && dimension(20).is_some_and(|height| SHEET_HEIGHTS.contains(&height))
}

/// PNG or WebP with the pet sheet layout.
fn is_sheet(bytes: &[u8]) -> bool {
    is_sheet_png(bytes) || (is_webp(bytes) && webp_size(bytes).is_some_and(|(width, height)| width == SHEET_WIDTH && SHEET_HEIGHTS.contains(&height)))
}

/// `displayName` of a pet folder's `pet.json`, when it has a usable one.
fn display_name(dir: &std::path::Path) -> Option<String> {
    let meta: Value = serde_json::from_str(&fs::read_to_string(dir.join("pet.json")).ok()?).ok()?;
    let name = meta["displayName"].as_str()?.trim();
    (!name.is_empty() && name.chars().count() <= 60).then(|| name.to_owned())
}

/// Every valid pet in the app's pets folder.
pub(crate) async fn list(State(state): State<AppState>) -> Json<Value> {
    let entries = pets_dir(&state).and_then(|dir| fs::read_dir(dir).ok());
    let pets: Vec<Value> = entries
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            (entry.path().is_dir() && installed(&state, &id).is_some())
                .then(|| json!({ "id": id, "name": display_name(&entry.path()).unwrap_or_else(|| id.clone()) }))
        })
        .collect();
    Json(json!({ "pets": pets }))
}

#[derive(serde::Deserialize)]
pub(crate) struct ImportRequest {
    path: String,
}

/// Imports the user's own pet folder (`pet.json` + sprite sheet) into the app's pets folder. The folder is read, never changed.
pub(crate) async fn import(State(state): State<AppState>, Json(request): Json<ImportRequest>) -> ApiResult<Json<Value>> {
    let bad = |message: &'static str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let dir = PathBuf::from(request.path.trim());
    let meta: Value = fs::read_to_string(dir.join("pet.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(|| bad("Thư mục không có pet.json hợp lệ"))?;
    let id = meta["id"].as_str().unwrap_or_default();
    if !safe_name(id) {
        return Err(bad("Mã pet trong pet.json không hợp lệ (chỉ a-z, 0-9 và -)"));
    }
    // The sheet must sit in the same folder: a bare file name, never a path.
    let file = meta["spritesheetPath"].as_str().filter(|name| std::path::Path::new(name).file_name().is_some_and(|f| f == *name));
    let bytes = fs::read(dir.join(file.unwrap_or("spritesheet.png")))
        .ok()
        .filter(|bytes| bytes.len() <= MAX_BYTES && is_sheet(bytes))
        .ok_or_else(|| bad("Không đọc được ảnh sprite (cần PNG/WebP rộng 1536 px, cao 1248 px (6 hàng) hoặc 1040 px (5 hàng), tối đa 4 MB)"))?;
    let name = display_name(&dir).unwrap_or_else(|| id.to_owned());
    save(&state, id, &name, &bytes)?;
    Ok(Json(json!({ "id": id, "name": name })))
}

/// Writes a validated sprite sheet and its `pet.json` into the app's pets folder.
fn save(state: &AppState, id: &str, name: &str, bytes: &[u8]) -> ApiResult<()> {
    let target = downloaded_file(state, id).ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Mã pet không hợp lệ"))?;
    target
        .parent()
        .map(fs::create_dir_all)
        .transpose()
        .and_then(|_| fs::write(&target, bytes))
        .and_then(|_| fs::write(target.with_file_name("pet.json"), json!({"id": id, "displayName": name}).to_string()))
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được pet"))
}

/// Pixel size from a WebP header (VP8, VP8L or VP8X); `None` when it cannot be read.
fn webp_size(b: &[u8]) -> Option<(u32, u32)> {
    let le = |at: usize, n: usize| b.get(at..at + n).map(|s| s.iter().rev().fold(0u32, |a, &x| a << 8 | u32::from(x)));
    match b.get(12..16)? {
        b"VP8X" => Some((le(24, 3)? + 1, le(27, 3)? + 1)),
        b"VP8L" => {
            let bits = le(21, 4)?;
            (b[20] == 0x2f).then(|| ((bits & 0x3fff) + 1, (bits >> 14 & 0x3fff) + 1))
        }
        b"VP8 " => Some((le(26, 2)? & 0x3fff, le(28, 2)? & 0x3fff)),
        _ => None,
    }
}

#[derive(serde::Deserialize)]
pub(crate) struct UploadQuery {
    /// File name the user picked, e.g. `my pet.webp`.
    name: String,
}

/// Adds a sprite sheet the user picked in Settings (raw WebP body). Stored as `my-<file name>`; uploading the same name again replaces it.
pub(crate) async fn upload(State(state): State<AppState>, Query(query): Query<UploadQuery>, body: Bytes) -> ApiResult<Json<Value>> {
    let bad = |message: &'static str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let stem = query.name.rsplit_once('.').map_or(query.name.as_str(), |(stem, _)| stem).trim();
    let slug: String = stem.to_lowercase().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        return Err(bad("Tên file không hợp lệ"));
    }
    let id: String = format!("my-{slug}").chars().take(48).collect();
    if body.len() > MAX_BYTES || !is_webp(&body) || !is_sheet(&body) {
        return Err(bad("Cần ảnh WebP rộng 1536 px, cao 1248 px (8 cột × 6 hàng) hoặc 1040 px (5 hàng), tối đa 4 MB"));
    }
    let name: String = stem.chars().take(60).collect();
    save(&state, &id, &name, &body)?;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub(crate) async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let path = downloaded_file(&state, &id).ok_or_else(not_found)?;
    let _ = fs::remove_file(&path);
    let _ = fs::remove_file(path.with_file_name("pet.json"));
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir(dir);
    }
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pet_names_cannot_leave_the_pets_folder() {
        assert!(!safe_name("../../.ssh"));
        assert!(!safe_name("Clawd"));
        assert!(safe_name("clawd-svg"));
    }

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes
    }

    #[test]
    fn only_pet_sheet_pngs_are_accepted() {
        assert!(is_sheet_png(&png(1536, 1248)));
        assert!(is_sheet_png(&png(1536, 1040)));
        assert!(!is_sheet_png(&png(1536, 1872)));
        assert!(!is_sheet_png(&png(800, 1248)));
        assert!(!is_sheet_png(&png(1536, 1456)));
        assert!(!is_sheet_png(&png(1536, 1041)));
        assert!(!is_sheet_png(b"<html>not an image</html>"));
    }

    #[test]
    fn webp_size_is_read_from_the_header() {
        let (w, h) = (1535u32.to_le_bytes(), 1871u32.to_le_bytes());
        let mut vp8x = b"RIFF\0\0\0\0WEBPVP8X\n\0\0\0\0\0\0\0".to_vec();
        vp8x.extend(&w[..3]);
        vp8x.extend(&h[..3]);
        assert_eq!(webp_size(&vp8x), Some((1536, 1872)));
        let mut vp8l = b"RIFF\0\0\0\0WEBPVP8L\0\0\0\0\x2f".to_vec();
        vp8l.extend((1535u32 | 1871 << 14).to_le_bytes());
        assert_eq!(webp_size(&vp8l), Some((1536, 1872)));
        assert_eq!(webp_size(b"RIFF\0\0\0\0WEBPjunk"), None);
    }
}
