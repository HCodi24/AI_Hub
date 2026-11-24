use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    mutis::{record_plan_event, Event, PlanEventError, PlanEventInput},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde_json::{json, Value};
use std::collections::HashMap;

pub(crate) async fn record_mutis_event(
    State(state): State<AppState>,
    Json(input): Json<PlanEventInput>,
) -> ApiResult<Json<Value>> {
    let config = state.config.read().await;
    if !config.mutis.effective(config.mutis.event_log) {
        return Ok(Json(json!({"ok": true, "skipped": true})));
    }
    record_plan_event(&state.event_log, input).map_err(|error| match error {
        PlanEventError::InvalidKind(message) => ApiError::new(StatusCode::BAD_REQUEST, message),
        PlanEventError::AppendFailed => ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể ghi nhật ký MutiS",
        ),
    })?;
    Ok(Json(json!({"ok": true})))
}

pub(crate) async fn mutis_events(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<Event>>> {
    let limit = query
        .get("limit")
        .and_then(|x| x.parse().ok())
        .unwrap_or(200)
        .min(2000);
    state.event_log.recent(limit).map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được nhật ký MutiS",
        )
    })
}
