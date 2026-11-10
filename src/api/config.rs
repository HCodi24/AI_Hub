use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config},
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct ConfigInput {
    default_folder: String,
    #[serde(default)]
    planner_provider_id: String,
    #[serde(default)]
    planner_model: String,
}

pub(crate) async fn get_config(State(state): State<AppState>) -> Json<Config> {
    let mut config = state.config.read().await.clone();
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    Json(config)
}

pub(crate) async fn put_config(
    State(state): State<AppState>,
    Json(input): Json<ConfigInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    config.default_folder = input.default_folder.trim().to_owned();
    config.planner_provider_id = input.planner_provider_id;
    config.planner_model = input.planner_model;
    config.mutis.roles.lead.provider_id = config.planner_provider_id.clone();
    config.mutis.roles.lead.model = config.planner_model.clone();
    config.mutis.reviewer_provider_id = config.planner_provider_id.clone();
    config.mutis.reviewer_model = config.planner_model.clone();
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    let mut response = config.clone();
    for provider in &mut response.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    Ok(Json(response))
}
