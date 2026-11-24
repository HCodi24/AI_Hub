use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config, MutisConfig},
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
    let provider_ids: Vec<_> = config.providers.iter().map(|p| p.id.clone()).collect();
    validate(&value, &provider_ids)?;

    let mut next = config.clone();
    next.mutis = value.clone();
    sync_lead(&mut next);
    config::save(&state.config_path, &next)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    *config = next;
    Ok(Json(value))
}

pub(super) fn validate(value: &MutisConfig, provider_ids: &[String]) -> ApiResult<()> {
    if !(1..=5).contains(&value.retry_attempts)
        || !(1..=60).contains(&value.watchdog_minutes)
        || value.resume_max > 3
        || value.fix_rounds_max > 5
        || !(50..=5000).contains(&value.deepseek_max_file_lines)
        || !(5..=40).contains(&value.deepseek_max_steps)
        || !(1..=3).contains(&value.debate_rounds)
        || value.retry_base_ms > 60000
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Giá trị ngoài giới hạn",
        ));
    }
    for rule in &value.routing {
        if !matches!(
            rule.kind.as_str(),
            "small_edit" | "code" | "test" | "docs" | "plan" | "review"
        ) || !matches!(rule.role.as_str(), "lead" | "big" | "small")
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Quy tắc định tuyến không hợp lệ",
            ));
        }
    }
    for role in [
        &value.roles.lead,
        &value.roles.big,
        &value.roles.small,
        &value.high_roles.lead,
        &value.high_roles.big,
        &value.high_roles.small,
    ] {
        if !role.provider_id.is_empty() && !provider_ids.contains(&role.provider_id) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Nguồn AI không tồn tại",
            ));
        }
        // A default, unconfigured role has no effort override.
        if role.provider_id.is_empty() && role.model.is_empty() && role.effort.is_empty() {
            continue;
        }
        if !matches!(
            role.effort.as_str(),
            "default" | "low" | "medium" | "high" | "xhigh" | "max"
        ) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Mức suy luận không hợp lệ",
            ));
        }
    }
    Ok(())
}

pub(super) fn sync_lead(config: &mut Config) {
    let lead = if config.mutis.high_roles.lead.provider_id.trim().is_empty() {
        &config.mutis.roles.lead
    } else {
        &config.mutis.high_roles.lead
    };
    config.planner_provider_id = lead.provider_id.clone();
    config.planner_model = lead.model.clone();
    config.mutis.reviewer_provider_id = lead.provider_id.clone();
    config.mutis.reviewer_model = lead.model.clone();
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
    if matches!(
        change.path.as_str(),
        "mutis.roles.lead.provider_id"
            | "mutis.roles.lead.model"
            | "mutis.high_roles.lead.provider_id"
            | "mutis.high_roles.lead.model"
    ) {
        sync_lead(&mut next);
    }
    config::save(&state.config_path, &next)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cấu hình"))?;
    *config = next;
    Ok(Json(json!({"old":old,"new":new})))
}
