use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, MutisConfig},
    mutis,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

pub(crate) async fn get_mutis(State(state): State<AppState>) -> Json<MutisConfig> {
    Json(state.config.read().await.mutis.clone())
}

pub(crate) async fn put_mutis(
    State(state): State<AppState>,
    Json(value): Json<MutisConfig>,
) -> ApiResult<Json<MutisConfig>> {
    let mut config = state.config.write().await;
    let mut next = config.clone();
    mutis::replace_settings(&mut next, &value)
        .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
    config::save(&state.config_path, &next)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    *config = next;
    Ok(Json(value))
}

#[derive(Deserialize)]
pub(crate) struct SettingChange {
    path: String,
    value: Value,
}
pub(crate) async fn mutis_setting_change(
    State(state): State<AppState>,
    Json(change): Json<SettingChange>,
) -> ApiResult<Json<Value>> {
    let mut config = state.config.write().await;
    let mut next = config.clone();
    let (old, new) = mutis::apply_setting_change(&mut next, &change.path, &change.value)
        .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
    config::save(&state.config_path, &next)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cấu hình"))?;
    *config = next;
    Ok(Json(json!({"old":old,"new":new})))
}
