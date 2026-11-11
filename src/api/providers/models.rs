//! Model list of a source, and the usage limits of a CLI source.
use super::crud::persist_config;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    models: Vec<String>,
}

pub(crate) async fn provider_limits(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<providers::limits::Limits>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Provider not found"))?;
    Ok(Json(providers::limits::read(&provider).await))
}

pub(crate) async fn provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ModelsResponse>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    if provider.kind != "openai_compat" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho OpenAI-compatible",
        ));
    }
    if provider.api_key.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy lưu API key trước khi tải model",
        ));
    }
    let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Không thể kết nối tới API models"))?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("API models trả về HTTP {}", response.status()),
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Phản hồi models không hợp lệ"))?;
    let mut models = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    {
        let mut config = state.config.write().await;
        if let Some(entry) = config.providers.iter_mut().find(|entry| entry.id == id) {
            entry.models = models.clone();
            persist_config(&state, &config)?;
        }
    }
    Ok(Json(ModelsResponse { models }))
}
