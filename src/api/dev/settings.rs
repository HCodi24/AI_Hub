//! Dev-mode status and settings endpoints.

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config,
};
use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

pub(crate) async fn dev_status(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"dev":state.dev_repo.is_some()}))
}

pub(crate) async fn dev_settings(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"auto_rebuild":state.config.read().await.self_dev.auto_rebuild}))
}

pub(crate) async fn put_dev_settings(
    State(state): State<AppState>,
    Json(value): Json<config::SelfDevConfig>,
) -> ApiResult<Json<config::SelfDevConfig>> {
    if state.dev_repo.is_none() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Chế độ phát triển đang tắt",
        ));
    }
    let mut cfg = state.config.write().await;
    cfg.self_dev = value.clone();
    config::save(&state.config_path, &cfg)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cài đặt"))?;
    Ok(Json(value))
}
