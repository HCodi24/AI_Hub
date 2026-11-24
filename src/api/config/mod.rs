mod apply;
mod input;
mod validate;
#[cfg(test)]
mod tests;

use self::{apply::apply, input::ConfigInput, validate::validate};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config},
};
use axum::{extract::State, http::StatusCode, Json};

pub(crate) async fn get_config(State(state): State<AppState>) -> Json<Config> {
    Json(masked(state.config.read().await.clone()))
}

pub(crate) async fn put_config(
    State(state): State<AppState>,
    Json(input): Json<ConfigInput>,
) -> ApiResult<Json<Config>> {
    validate(&input)?;
    let mut stored = state.config.write().await;
    let mut config = stored.clone();
    apply(&mut config, input);
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    *stored = config;
    Ok(Json(masked(stored.clone())))
}

fn masked(mut config: Config) -> Config {
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    config
}
