//! Pet sprite sheets for the chat avatars. None ship with this app: the user uploads a WebP sheet or imports a pet folder in Settings,
//! and each pet lives in `<app data>/pets/<id>/`.
mod sheet;
mod store;

use self::sheet::{is_png, is_sheet, is_webp, MAX_BYTES};
#[cfg(test)]
use self::sheet::{is_sheet_png, webp_size};
use self::store::{
    display_name, downloaded_file, installed, is_installed, pets_dir, remove_files, safe_name, save,
    upload_id,
};
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

const IMPORT_SHEET_ERROR: &str = "Không đọc được ảnh sprite (cần PNG/WebP rộng 1536 px, cao 1248 px (6 hàng) hoặc 1040 px (5 hàng), tối đa 4 MB)";
const UPLOAD_SHEET_ERROR: &str =
    "Cần ảnh WebP rộng 1536 px, cao 1248 px (8 cột × 6 hàng) hoặc 1040 px (5 hàng), tối đa 4 MB";

fn not_found() -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, "Không có pet này")
}

pub(crate) async fn sprite(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> ApiResult<Response> {
    let bytes = installed(&state, &name).ok_or_else(not_found)?;
    Ok(image(bytes, "max-age=60"))
}

fn image(bytes: Vec<u8>, cache: &'static str) -> Response {
    let mime = if is_png(&bytes) {
        "image/png"
    } else {
        "image/webp"
    };
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
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
            (entry.path().is_dir() && is_installed(&state, &id))
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
pub(crate) async fn import(
    State(state): State<AppState>,
    Json(request): Json<ImportRequest>,
) -> ApiResult<Json<Value>> {
    let bad = |message: &'static str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let dir = PathBuf::from(request.path.trim());
    let meta: Value = fs::read_to_string(dir.join("pet.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .ok_or_else(|| bad("Thư mục không có pet.json hợp lệ"))?;
    let id = meta["id"].as_str().unwrap_or_default();
    if !safe_name(id) {
        return Err(bad(
            "Mã pet trong pet.json không hợp lệ (chỉ a-z, 0-9 và -)",
        ));
    }
    // The sheet must sit in the same folder: a bare file name, never a path.
    let file = meta["spritesheetPath"].as_str().filter(|name| {
        std::path::Path::new(name)
            .file_name()
            .is_some_and(|f| f == *name)
    });
    let bytes = fs::read(dir.join(file.unwrap_or("spritesheet.png")))
        .ok()
        .filter(|bytes| bytes.len() <= MAX_BYTES && is_sheet(bytes))
        .ok_or_else(|| bad(IMPORT_SHEET_ERROR))?;
    let name = display_name(&dir).unwrap_or_else(|| id.to_owned());
    save(&state, id, &name, &bytes)?;
    Ok(Json(json!({ "id": id, "name": name })))
}

#[derive(serde::Deserialize)]
pub(crate) struct UploadQuery {
    /// File name the user picked, e.g. `my pet.webp`.
    name: String,
}

/// Adds a sprite sheet the user picked in Settings (raw WebP body). Stored as `my-<file name>`; uploading the same name again replaces it.
pub(crate) async fn upload(
    State(state): State<AppState>,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> ApiResult<Json<Value>> {
    let bad = |message: &'static str| ApiError::new(StatusCode::BAD_REQUEST, message);
    let stem = query
        .name
        .rsplit_once('.')
        .map_or(query.name.as_str(), |(stem, _)| stem);
    let id = upload_id(stem).ok_or_else(|| bad("Tên file không hợp lệ"))?;
    if body.len() > MAX_BYTES || !is_webp(&body) || !is_sheet(&body) {
        return Err(bad(UPLOAD_SHEET_ERROR));
    }
    let name: String = stem.trim().chars().take(60).collect();
    save(&state, &id, &name, &body)?;
    Ok(Json(json!({ "id": id, "name": name })))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let path = downloaded_file(&state, &id).ok_or_else(not_found)?;
    // Deletion is idempotent, so file removal errors are intentionally ignored.
    remove_files(&path);
    Ok(Json(json!({ "ok": true })))
}

#[cfg(test)]
mod tests;
