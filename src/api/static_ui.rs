use crate::app::AppState;
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use std::path::{Component, Path as FsPath};

include!(concat!(env!("OUT_DIR"), "/ui_assets.rs"));

pub(crate) async fn index(State(state): State<AppState>) -> Response {
    serve_asset(&state, "index.html").await
}

pub(crate) async fn asset(State(state): State<AppState>, Path(path): Path<String>) -> Response {
    if !safe_ui_path(&path) {
        return StatusCode::NOT_FOUND.into_response();
    }
    serve_asset(&state, &path).await
}

fn safe_ui_path(path: &str) -> bool {
    let path = FsPath::new(path);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        && matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("html" | "js" | "css" | "svg" | "png")
        )
}

async fn serve_asset(state: &AppState, path: &str) -> Response {
    let Some(content_type) = content_type(path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if let Some(repo) = state.dev_repo.as_ref() {
        if !safe_ui_path(path) {
            return StatusCode::NOT_FOUND.into_response();
        }
        let file = repo.join("ui").join(path);
        return match tokio::fs::read(file).await {
            Ok(body) => (
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                body,
            )
                .into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        };
    }
    match UI_ASSETS.iter().find(|(name, _)| *name == path) {
        Some((_, body)) => ([(header::CONTENT_TYPE, content_type)], *body).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn content_type(path: &str) -> Option<&'static str> {
    match FsPath::new(path).extension()?.to_str()? {
        "html" => Some("text/html; charset=utf-8"),
        "js" => Some("text/javascript; charset=utf-8"),
        "css" => Some("text/css; charset=utf-8"),
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        _ => None,
    }
}
