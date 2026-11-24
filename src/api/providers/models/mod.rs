//! Model list of a source, and the usage limits of a CLI source.
mod catalog;
mod claude;
mod codex;
mod openai;

use self::{claude::claude_models, codex::codex_models, openai::openai_models};
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
use std::collections::BTreeMap;

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
    if matches!(provider.kind.as_str(), "codex_cli" | "claude_cli" | "antigravity_cli") {
        let (models, efforts) = if provider.kind == "claude_cli" {
            let catalog = claude_models(&state, &provider).await?;
            (catalog.ids, catalog.efforts)
        } else if provider.kind == "antigravity_cli" {
            (super::antigravity::list_models(&provider).await?, BTreeMap::new())
        } else {
            let catalog = codex_models(&state, &provider).await?;
            (catalog.ids, catalog.efforts)
        };
        let mut config = state.config.write().await;
        let mut next = config.clone();
        let entry = next.providers.iter_mut().find(|entry| entry.id == id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
        if (entry.kind.as_str(), entry.executable.as_str(), entry.web_login)
            != (provider.kind.as_str(), provider.executable.as_str(), provider.web_login)
        {
            return Err(ApiError::new(StatusCode::CONFLICT, "Cấu hình nguồn đã thay đổi. Hãy tải lại danh sách model."));
        }
        entry.models = models.clone();
        entry.model_efforts = efforts;
        persist_config(&state, &next)?;
        *config = next;
        return Ok(Json(ModelsResponse { models }));
    }
    if provider.kind != "openai_compat" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho Codex CLI, Claude Code CLI và OpenAI-compatible",
        ));
    }
    let models = openai_models(&state, &provider).await?;
    Ok(Json(ModelsResponse { models }))
}

#[cfg(test)]
mod tests;
