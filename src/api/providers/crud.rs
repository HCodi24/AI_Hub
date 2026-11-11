//! Add, edit and remove AI sources, plus the validation and persistence they share.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config, ProviderConfig},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ProviderInput {
    #[serde(default)]
    id: Option<String>,
    name: String,
    kind: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    executable: String,
    #[serde(default)]
    models: Vec<String>,
    /// None = keep the stored value (partial updates must not reset it).
    #[serde(default)]
    agent_capable: Option<bool>,
}

pub(crate) async fn create_provider(
    State(state): State<AppState>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_provider(&id, &input)?;
    if config.providers.iter().any(|provider| provider.id == id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Mã nhà cung cấp đã tồn tại",
        ));
    }
    let kind_is_openai = input.kind == "openai_compat";
    let provider = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key: input.api_key.unwrap_or_default(),
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
        agent_capable: kind_is_openai && input.agent_capable.unwrap_or(false),
    };
    config.providers.push(provider);
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    validate_provider(&id, &input)?;
    let mut config = state.config.write().await;
    let Some(index) = config
        .providers
        .iter()
        .position(|provider| provider.id == id)
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    };
    let old_key = config.providers[index].api_key.clone();
    let old_agent_capable = config.providers[index].agent_capable;
    let submitted_key = input.api_key.unwrap_or_default();
    let api_key = if submitted_key.trim().is_empty() || config::is_masked_key(&submitted_key) {
        old_key
    } else {
        submitted_key
    };
    let kind_is_openai = input.kind == "openai_compat";
    config.providers[index] = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key,
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
        agent_capable: kind_is_openai && input.agent_capable.unwrap_or(old_agent_capable),
    };
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn remove_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let previous_len = config.providers.len();
    config.providers.retain(|provider| provider.id != id);
    if config.providers.len() == previous_len {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    }
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

fn validate_provider(id: &str, input: &ProviderInput) -> ApiResult<()> {
    if !valid_provider_id(id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã nhà cung cấp không hợp lệ",
        ));
    }
    if input.name.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tên nhà cung cấp không được để trống",
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "openai_compat" | "anthropic" | "codex_cli" | "claude_cli"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nhà cung cấp không hợp lệ",
        ));
    }
    if input.kind == "openai_compat" && input.base_url.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "OpenAI-compatible cần URL cơ sở",
        ));
    }
    Ok(())
}

fn valid_provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn clean_models(models: Vec<String>) -> Vec<String> {
    models
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect()
}

fn masked_config(config: &Config) -> Config {
    let mut config = config.clone();
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    config
}

pub(super) fn persist_config(state: &AppState, config: &Config) -> ApiResult<()> {
    config::save(&state.config_path, config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))
}
