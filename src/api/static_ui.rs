use crate::app::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

pub(crate) async fn index(State(state): State<AppState>) -> Response {
    if let Some(repo) = state.dev_repo.as_ref() {
        return asset_response(&repo.join("ui/index.html"), "text/html; charset=utf-8").await;
    }
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../../ui/index.html"),
    )
        .into_response()
}

pub(crate) async fn app_js(State(state): State<AppState>) -> Response {
    if let Some(repo) = state.dev_repo.as_ref() {
        return asset_response(&repo.join("ui/app.js"), "text/javascript; charset=utf-8").await;
    }
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        include_str!("../../ui/app.js"),
    )
        .into_response()
}

pub(crate) async fn style_css(State(state): State<AppState>) -> Response {
    if let Some(repo) = state.dev_repo.as_ref() {
        return asset_response(&repo.join("ui/style.css"), "text/css; charset=utf-8").await;
    }
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../../ui/style.css"),
    )
        .into_response()
}

async fn asset_response(path: &std::path::Path, content_type: &'static str) -> Response {
    match tokio::fs::read(path).await {
        Ok(body) => (
            [
                (axum::http::header::CONTENT_TYPE, content_type),
                (axum::http::header::CACHE_CONTROL, "no-store"),
            ],
            body,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Dev UI file not found").into_response(),
    }
}
