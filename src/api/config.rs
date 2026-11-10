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
    // Optional on purpose: a partial update (e.g. the UI saving only the working folder) must NOT reset
    // settings it did not send. Only fields that are present are applied.
    #[serde(default)]
    planner_provider_id: Option<String>,
    #[serde(default)]
    planner_model: Option<String>,
    #[serde(default)]
    chat_idle_timeout_minutes: Option<u32>,
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
    if input
        .chat_idle_timeout_minutes
        .is_some_and(|minutes| minutes > 120)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thời gian chờ chat phải từ 0 đến 120 phút",
        ));
    }
    let mut config = state.config.write().await;
    config.default_folder = input.default_folder.trim().to_owned();
    if let Some(minutes) = input.chat_idle_timeout_minutes {
        config.chat_idle_timeout_minutes = minutes;
    }
    if let Some(provider_id) = input.planner_provider_id {
        config.planner_provider_id = provider_id;
        config.mutis.roles.lead.provider_id = config.planner_provider_id.clone();
        config.mutis.reviewer_provider_id = config.planner_provider_id.clone();
    }
    if let Some(model) = input.planner_model {
        config.planner_model = model;
        config.mutis.roles.lead.model = config.planner_model.clone();
        config.mutis.reviewer_model = config.planner_model.clone();
    }
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    let mut response = config.clone();
    for provider in &mut response.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::ConfigInput;

    #[test]
    fn partial_update_only_carries_fields_that_were_sent() {
        let input: ConfigInput = serde_json::from_str(r#"{"default_folder":"C:/TOOL"}"#).unwrap();
        assert_eq!(input.default_folder, "C:/TOOL");
        assert!(input.planner_provider_id.is_none());
        assert!(input.planner_model.is_none());
        assert!(input.chat_idle_timeout_minutes.is_none());
        let full: ConfigInput =
            serde_json::from_str(r#"{"default_folder":"x","chat_idle_timeout_minutes":0}"#)
                .unwrap();
        assert_eq!(full.chat_idle_timeout_minutes, Some(0));
    }
}
