use crate::app::AppState;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::Path as FsPath;

include!(concat!(env!("OUT_DIR"), "/web_assets.rs"));

/// `/` opens the React UI (served under /app/).
pub(crate) async fn index() -> Response {
    axum::response::Redirect::temporary("/app/").into_response()
}

fn web_roots(state: &AppState) -> Vec<std::path::PathBuf> {
    let root = state
        .dev_repo
        .as_ref()
        .map(|repo| repo.join("web/out"))
        .or_else(|| std::env::current_dir().ok().map(|dir| dir.join("web/out")));
    let exe_root = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("web/out")));
    [root, exe_root].into_iter().flatten().collect()
}

fn content_type(path: &str) -> Option<&'static str> {
    match FsPath::new(path).extension()?.to_str()? {
        "html" => Some("text/html; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "woff2" => Some("font/woff2"),
        "json" => Some("application/json; charset=utf-8"),
        "txt" => Some("text/plain; charset=utf-8"),
        "ico" => Some("image/x-icon"),
        "webp" => Some("image/webp"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "map" => Some("application/json; charset=utf-8"),
        _ => None,
    }
}

pub(crate) async fn web_index(State(state): State<AppState>) -> Response {
    serve_web(&state, "").await
}

pub(crate) async fn web_asset(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    serve_web(&state, &path).await
}

fn web_candidates(path: &str) -> Option<Vec<String>> {
    // Reject Windows separators and traversal even when built on Unix.
    if path.starts_with('/')
        || path.contains('\\')
        || path.contains(':')
        || path.split('/').any(|part| part == ".." || part == ".")
    {
        return None;
    }
    let path = path.trim_end_matches('/');
    if FsPath::new(path).is_absolute() {
        return None;
    }
    let mut candidates = Vec::new();
    if !path.is_empty() {
        candidates.push(path.to_owned());
        candidates.push(format!("{path}/index.html"));
    } else {
        candidates.push("index.html".to_owned());
    }
    candidates.push("404.html".to_owned());
    candidates.push("index.html".to_owned());
    Some(candidates)
}

async fn serve_web(state: &AppState, path: &str) -> Response {
    let Some(candidates) = web_candidates(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    for root in web_roots(state) {
        let Ok(canonical_root) = tokio::fs::canonicalize(&root).await else {
            continue;
        };
        for candidate in &candidates {
            let Ok(file) = tokio::fs::canonicalize(root.join(candidate)).await else {
                continue;
            };
            if !file.starts_with(&canonical_root) {
                continue;
            }
            if let Ok(body) = tokio::fs::read(file).await {
                return web_response(candidate, body);
            }
        }
    }
    if state.dev_repo.is_none() {
        for candidate in &candidates {
            if let Some((_, body)) = WEB_ASSETS.iter().find(|(name, _)| name == candidate) {
                return web_response(candidate, body.to_vec());
            }
        }
    }
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<!doctype html><html lang=\"vi\"><meta charset=\"utf-8\"><meta http-equiv=\"refresh\" content=\"3\"><title>AI Hub</title>\
         <p>Giao diện mới chưa được build: chạy npm run build trong web/</p></html>",
    )
        .into_response()
}

fn web_response(path: &str, body: Vec<u8>) -> Response {
    (
        [
            (
                header::CONTENT_TYPE,
                content_type(path).unwrap_or("application/octet-stream"),
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod web_tests {
    use super::*;

    #[test]
    fn export_routes_and_traversal() {
        assert_eq!(web_candidates("").unwrap()[0], "index.html");
        assert_eq!(web_candidates("chat/").unwrap()[1], "chat/index.html");
        assert!(web_candidates("../config.json").is_none());
        assert!(web_candidates("..\\config.json").is_none());
        assert!(web_candidates("C:/config.json").is_none());
        assert!(web_candidates("/etc/passwd").is_none());
        assert_eq!(
            content_type("bundle.js"),
            Some("text/javascript; charset=utf-8")
        );
        assert_eq!(content_type("font.woff2"), Some("font/woff2"));
    }
}
