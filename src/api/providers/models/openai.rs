use super::super::crud::persist_config;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
};
use axum::http::StatusCode;
use serde_json::Value;
use std::time::Duration;

pub(super) async fn openai_models(
    state: &AppState,
    provider: &ProviderConfig,
) -> ApiResult<Vec<String>> {
    if provider.api_key.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy lưu API key trước khi tải model",
        ));
    }
    let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
    let connection_error = || {
        ApiError::new(StatusCode::BAD_GATEWAY, "Không thể kết nối tới API models")
    };
    let response = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|_| connection_error())?
        .get(url)
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|_| connection_error())?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("API models trả về HTTP {}", response.status()),
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|error| {
            if error.is_timeout() || error.is_connect() {
                connection_error()
            } else {
                ApiError::new(StatusCode::BAD_GATEWAY, "Phản hồi models không hợp lệ")
            }
        })?;
    let models = parse_model_ids(&body);
    {
        let mut config = state.config.write().await;
        let mut next = config.clone();
        if let Some(entry) = next.providers.iter_mut().find(|entry| entry.id == provider.id) {
            if (&entry.kind, &entry.base_url, &entry.api_key)
                != (&provider.kind, &provider.base_url, &provider.api_key)
            {
                return Err(ApiError::new(
                    StatusCode::CONFLICT,
                    "Cấu hình nguồn đã thay đổi. Hãy tải lại danh sách model.",
                ));
            }
            entry.models = models.clone();
            entry.model_efforts.clear();
            persist_config(state, &next)?;
            *config = next;
        }
    }
    Ok(models)
}

pub(super) fn parse_model_ids(body: &Value) -> Vec<String> {
    let mut models = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    models
}
