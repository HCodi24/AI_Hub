use crate::api::{ApiError, ApiResult};
use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// One level of a folder, folders first, so the UI can expand the tree lazily.
/// shortcut: any readable path is listed (local single-user app), restrict to the chat folder if the server is ever exposed.
pub(crate) async fn list_dir(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    let path = q.get("path").cloned().unwrap_or_default();
    let entries = sorted_entries(&path)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Không đọc được thư mục"))?;
    Ok(Json(
        json!({ "entries": entries.into_iter().map(|(dir, name)| json!({ "name": name, "dir": dir })).collect::<Vec<_>>() }),
    ))
}

pub(super) fn sorted_entries(path: &str) -> std::io::Result<Vec<(bool, String)>> {
    let mut entries: Vec<(bool, String)> = std::fs::read_dir(path)?
        .flatten()
        .map(|e| {
            (
                e.path().is_dir(),
                e.file_name().to_string_lossy().into_owned(),
            )
        })
        .collect();
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then(a.1.to_lowercase().cmp(&b.1.to_lowercase()))
    });
    Ok(entries)
}
