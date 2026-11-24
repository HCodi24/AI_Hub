//! Add, edit and remove AI sources, plus the validation and persistence they share.
mod validate;

use self::validate::{clean_models, validate_provider};
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
    /// CLI sources: sign in through the web instead of the machine's CLI account. None = keep the stored value.
    #[serde(default)]
    web_login: Option<bool>,
}

fn normalize_input(mut input: ProviderInput) -> ProviderInput {
    input.name = input.name.trim().to_owned();
    input.base_url = input.base_url.trim().trim_end_matches('/').to_owned();
    input.executable = input.executable.trim().to_owned();
    input.models = clean_models(input.models);
    input
}

pub(crate) async fn create_provider(
    State(state): State<AppState>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_provider(&id, &input)?;
    let input = normalize_input(input);
    let mut config = state.config.write().await;
    if config.providers.iter().any(|provider| provider.id == id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Mã nhà cung cấp đã tồn tại",
        ));
    }
    let kind_is_openai = input.kind == "openai_compat";
    let kind_is_cli = input.kind.ends_with("_cli");
    let provider = ProviderConfig {
        id,
        name: input.name,
        base_url: input.base_url,
        api_key: input.api_key.unwrap_or_default(),
        executable: input.executable,
        models: input.models,
        agent_capable: kind_is_openai && input.agent_capable.unwrap_or(false),
        web_login: kind_is_cli && input.web_login.unwrap_or(false),
        kind: input.kind,
        ..Default::default()
    };
    let mut next = config.clone();
    let index = next.providers.len();
    next.providers.push(provider);
    persist_config(&state, &next)?;
    crate::providers::antigravity_cli::sync_login_mode(&next.providers[index]);
    *config = next;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    validate_provider(&id, &input)?;
    let input = normalize_input(input);
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
    let old_web_login = config.providers[index].web_login;
    let kind_is_openai = input.kind == "openai_compat";
    let kind_is_cli = input.kind.ends_with("_cli");
    let executable = input.executable;
    let models = input.models;
    let web_login = kind_is_cli && input.web_login.unwrap_or(old_web_login);
    let old = &config.providers[index];
    let model_efforts = if old.kind == input.kind && old.executable == executable && old.web_login == web_login {
        old.model_efforts.iter().filter(|(id, _)| models.contains(id))
            .map(|(id, effort)| (id.clone(), effort.clone())).collect()
    } else {
        Default::default()
    };
    let mut next = config.clone();
    next.providers[index] = ProviderConfig {
        id,
        name: input.name,
        base_url: input.base_url,
        api_key,
        executable,
        models,
        model_efforts,
        agent_capable: kind_is_openai && input.agent_capable.unwrap_or(old_agent_capable),
        web_login,
        kind: input.kind,
    };
    persist_config(&state, &next)?;
    crate::providers::antigravity_cli::sync_login_mode(&next.providers[index]);
    *config = next;
    Ok(Json(masked_config(&config)))
}

pub(crate) async fn remove_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    if matches!(id.as_str(), "codex-cli" | "claude-cli" | "antigravity-cli") {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Không thể xoá nguồn CLI mặc định",
        ));
    }
    let mut config = state.config.write().await;
    let mut next = config.clone();
    next.providers.retain(|provider| provider.id != id);
    if next.providers.len() == config.providers.len() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    }
    persist_config(&state, &next)?;
    *config = next;
    let response = masked_config(&config);
    drop(config);

    {
        let mut active = state.claude_login.lock().await;
        if active.as_ref().is_some_and(|session| session.provider_id == id) {
            if let Some(mut session) = active.take() {
                let _ = session.child.kill().await;
            }
        }
    }
    super::codex::close_connection(&state, &id).await;
    state.provider_status.lock().await.remove(&id);
    Ok(Json(response))
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

#[cfg(test)]
mod tests;
