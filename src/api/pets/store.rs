use super::sheet::{is_sheet, MAX_BYTES};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::http::StatusCode;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

pub(super) fn safe_name(name: &str) -> bool {
    (1..=48).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

pub(super) fn upload_id(stem: &str) -> Option<String> {
    let trimmed = stem.trim();
    if trimmed.is_empty() {
        return None;
    }
    let slug: String = trimmed
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    if stem
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b' ' | b'-' | b'_'))
    {
        return (!slug.is_empty()).then(|| format!("my-{}", &slug[..slug.len().min(45)]));
    }

    // FNV-1a over the original UTF-8 bytes, using the low 24 bits for six hex digits.
    let hash = stem.bytes().fold(0x811c9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x01000193)
    }) & 0x00ff_ffff;
    if slug.is_empty() {
        Some(format!("my-{hash:06x}"))
    } else {
        let slug = &slug[..slug.len().min(38)];
        Some(format!("my-{slug}-{hash:06x}"))
    }
}

/// `<app data>/pets`: one folder per pet the user added.
pub(super) fn pets_dir(state: &AppState) -> Option<PathBuf> {
    crate::core::paths::data_dir(&state.config_path)
        .ok()
        .map(|dir| dir.join("pets"))
}

/// Where a pet's sprite sheet is kept (the name becomes a path segment, so it is checked first).
pub(super) fn downloaded_file(state: &AppState, id: &str) -> Option<PathBuf> {
    safe_name(id)
        .then(|| pets_dir(state))
        .flatten()
        .map(|dir| dir.join(id).join("spritesheet.png"))
}

/// The stored sheet of a pet, when it is still a valid one.
pub(super) fn installed(state: &AppState, name: &str) -> Option<Vec<u8>> {
    let bytes = fs::read(downloaded_file(state, name)?).ok()?;
    (bytes.len() <= MAX_BYTES && is_sheet(&bytes)).then_some(bytes)
}

pub(super) fn is_installed(state: &AppState, name: &str) -> bool {
    let Some(path) = downloaded_file(state, name) else {
        return false;
    };
    if !fs::metadata(&path).is_ok_and(|meta| meta.is_file() && meta.len() <= MAX_BYTES as u64) {
        return false;
    }
    let Ok(file) = fs::File::open(path) else {
        return false;
    };
    let mut header = Vec::with_capacity(32);
    file.take(32).read_to_end(&mut header).is_ok() && is_sheet(&header)
}

/// `displayName` of a pet folder's `pet.json`, when it has a usable one.
pub(super) fn display_name(dir: &std::path::Path) -> Option<String> {
    let meta: Value = serde_json::from_str(&fs::read_to_string(dir.join("pet.json")).ok()?).ok()?;
    let name = meta["displayName"].as_str()?.trim();
    (!name.is_empty() && name.chars().count() <= 60).then(|| name.to_owned())
}

/// Writes a validated sprite sheet and its `pet.json` into the app's pets folder.
pub(super) fn save(state: &AppState, id: &str, name: &str, bytes: &[u8]) -> ApiResult<()> {
    let target = downloaded_file(state, id)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Mã pet không hợp lệ"))?;
    let write_files = || -> io::Result<()> {
        if let Some(dir) = target.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(
            target.with_file_name("pet.json"),
            json!({"id": id, "displayName": name}).to_string(),
        )?;
        fs::write(&target, bytes)
    };
    write_files().map_err(|error| {
        eprintln!("Could not save pet {id}: {error}");
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được pet")
    })
}

pub(super) fn remove_files(path: &Path) {
    let _ = fs::remove_file(path);
    let _ = fs::remove_file(path.with_file_name("pet.json"));
    if let Some(dir) = path.parent() {
        let _ = fs::remove_dir(dir);
    }
}
