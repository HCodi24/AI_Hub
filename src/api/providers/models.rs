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
    let (provider, ask_anthropic) = {
        let config = state.config.read().await;
        let provider = config.providers.iter().find(|p| p.id == id).cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Provider not found"))?;
        (provider, config.claude_usage_api)
    };
    Ok(Json(providers::limits::read(&provider, ask_anthropic).await))
}

/// Visible model ids from the signed-in Codex app-server (`model/list`, paged); the process is always closed.
async fn codex_models(
    state: &AppState,
    provider: &crate::config::ProviderConfig,
) -> ApiResult<Vec<String>> {
    let mut client = super::codex::new_client(state, provider).await?;
    let listed = list_codex_models(&mut client).await;
    client.close().await;
    let models = listed.map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;
    if models.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Codex không trả model nào; hãy đăng nhập Codex rồi thử lại",
        ));
    }
    Ok(models)
}

async fn list_codex_models(client: &mut crate::providers::codex::transport::Client) -> anyhow::Result<Vec<String>> {
    let mut models = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let mut params = serde_json::json!({"limit": 100, "includeHidden": false});
        if let Some(cursor) = &cursor {
            params["cursor"] = Value::String(cursor.clone());
        }
        let page = client.call("model/list", params).await?;
        for row in page["data"].as_array().into_iter().flatten() {
            if row["hidden"].as_bool().unwrap_or(false) {
                continue;
            }
            if let Some(id) = row["id"].as_str().or_else(|| row["model"].as_str()) {
                let id = id.trim();
                if !id.is_empty() && !models.iter().any(|known| known == id) {
                    models.push(id.to_owned());
                }
            }
        }
        cursor = page["nextCursor"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(models)
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
    if provider.kind == "codex_cli" {
        let models = codex_models(&state, &provider).await?;
        let mut config = state.config.write().await;
        if let Some(entry) = config.providers.iter_mut().find(|entry| entry.id == id) {
            entry.models = models.clone();
            persist_config(&state, &config)?;
        }
        return Ok(Json(ModelsResponse { models }));
    }
    if provider.kind != "openai_compat" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho Codex CLI và OpenAI-compatible",
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
