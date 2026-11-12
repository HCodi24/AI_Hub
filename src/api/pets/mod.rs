//! Pet sprite sheets for the chat avatars. They are never shipped with this app:
//! - Codex pets are fetched from OpenAI's public pet CDN on first use (like the Codex TUI) and cached.
//! - Third-party pets (Clawd) are downloaded only when the user presses "Tải" in Settings › Tiện ích.
//! - Pets the user installed into Codex (`~/.codex/pets/<name>/spritesheet.png|webp`) are read from there.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Duration};

mod catalog;

/// Same cap as the Codex TUI.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Every sprite sheet is 8 columns of 192 px; rows are 208 px each.
const SHEET_WIDTH: u32 = 1536;
const CELL_HEIGHT: u32 = 208;

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "Không có pet này")
}

fn safe_name(name: &str) -> bool {
    (1..=48).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// `<app data>/pets`: Codex pet cache, plus one folder per downloaded third-party pet.
fn pets_dir(state: &AppState) -> Option<PathBuf> {
    crate::core::paths::data_dir(&state.config_path).ok().map(|dir| dir.join("pets"))
}

pub(crate) async fn sprite(State(state): State<AppState>, Path(name): Path<String>) -> ApiResult<Response> {
    let Some((file, url)) = catalog::sprite(&name) else {
        let bytes = installed(&state, &name).ok_or_else(not_found)?;
        return Ok(image(bytes, "max-age=60"));
    };
    let dir = pets_dir(&state).ok_or_else(not_found)?;
    let path = dir.join(&file);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(_) => {
            let bytes = download(&url).await.filter(|bytes| is_webp(bytes)).ok_or_else(not_found)?;
            // A failed cache write only means the next view downloads again.
            let _ = fs::create_dir_all(&dir).and_then(|_| fs::write(&path, &bytes));
            bytes
        }
    };
    Ok(image(bytes, "max-age=86400"))
}

fn image(bytes: Vec<u8>, cache: &'static str) -> Response {
    let mime = if is_png(&bytes) { "image/png" } else { "image/webp" };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

/// Where a downloaded third-party pet is kept.
fn downloaded_file(state: &AppState, id: &str) -> Option<PathBuf> {
    safe_name(id).then(|| pets_dir(state)).flatten().map(|dir| dir.join(id).join("spritesheet.png"))
}

/// A third-party pet: the copy the user downloaded here, else one they installed into Codex (read in place).
fn installed(state: &AppState, name: &str) -> Option<Vec<u8>> {
    // The name becomes a path segment: check it before touching the disk.
    if !safe_name(name) {
        return None;
    }
    let codex = dirs::home_dir()?.join(".codex").join("pets").join(name);
    let from_codex = || ["spritesheet.png", "spritesheet.webp"].iter().find_map(|file| fs::read(codex.join(file)).ok());
    let bytes = downloaded_file(state, name).and_then(|path| fs::read(path).ok()).or_else(from_codex)?;
    (bytes.len() <= MAX_BYTES && (is_png(&bytes) || is_webp(&bytes))).then_some(bytes)
}

fn is_png(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG")
}

fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP"
}

/// PNG with the pet sheet layout: 1536 px wide, a whole number of 208 px rows.
fn is_sheet_png(bytes: &[u8]) -> bool {
    let dimension = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    is_png(bytes)
        && dimension(16) == Some(SHEET_WIDTH)
        && dimension(20).is_some_and(|height| height >= CELL_HEIGHT * 9 && height % CELL_HEIGHT == 0)
}

async fn download(url: &str) -> Option<Vec<u8>> {
    let response = reqwest::Client::new().get(url).timeout(Duration::from_secs(30)).send().await.ok()?;
    if !response.status().is_success() || response.content_length().is_some_and(|n| n as usize > MAX_BYTES) {
        return None;
    }
    let bytes = response.bytes().await.ok()?;
    (bytes.len() <= MAX_BYTES).then(|| bytes.to_vec())
}

/// Pets the user can download, and whether each is available.
pub(crate) async fn list(State(state): State<AppState>) -> Json<Value> {
    let pets: Vec<Value> = catalog::EXTERNAL
        .iter()
        .map(|pet| {
            json!({
                "id": pet.id, "name": pet.name, "provider": pet.provider, "source": pet.source, "notice": pet.notice,
                "installed": installed(&state, pet.id).is_some(),
                // Only a copy downloaded here can be removed here; a Codex install is left alone.
                "removable": downloaded_file(&state, pet.id).is_some_and(|path| path.is_file()),
            })
        })
        .collect();
    Json(json!({ "pets": pets }))
}

pub(crate) async fn install(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let pet = catalog::external(&id).ok_or_else(not_found)?;
    let path = downloaded_file(&state, pet.id).ok_or_else(not_found)?;
    let bytes = download(pet.url)
        .await
        .filter(|bytes| is_sheet_png(bytes))
        .ok_or_else(|| ApiError::new(StatusCode::BAD_GATEWAY, "Không tải được pet (mạng lỗi hoặc ảnh không hợp lệ)"))?;
    let write = path
        .parent()
        .map(fs::create_dir_all)
        .transpose()
        .and_then(|_| fs::write(&path, &bytes));
    write.map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được pet"))?;
    Ok(Json(json!({ "ok": true })))
}

pub(crate) async fn remove(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let pet = catalog::external(&id).ok_or_else(not_found)?;
    if let Some(path) = downloaded_file(&state, pet.id) {
        // Only the downloaded copy goes: remove the file and its (then empty) folder, never anything under ~/.codex.
        let _ = fs::remove_file(&path);
        if let Some(dir) = path.parent() {
            let _ = fs::remove_dir(dir);
        }
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
        assert!(is_sheet_png(&png(1536, 2288)));
        assert!(is_sheet_png(&png(1536, 1872)));
        assert!(!is_sheet_png(&png(800, 2288)));
        assert!(!is_sheet_png(&png(1536, 2289)));
        assert!(!is_sheet_png(b"<html>not an image</html>"));
    }
}
