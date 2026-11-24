use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    mutis::{self, Event},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Deserialize)]
pub(crate) struct PlanEventInput {
    plan_id: String,
    task_id: String,
    provider_id: String,
    kind: String,
    #[serde(default)]
    task_kind: Option<String>,
    ok: bool,
    detail: Option<String>,
}
pub(crate) async fn record_mutis_event(
    State(state): State<AppState>,
    Json(input): Json<PlanEventInput>,
) -> ApiResult<Json<Value>> {
    let config = state.config.read().await;
    if !config.mutis.effective(config.mutis.event_log) {
        return Ok(Json(json!({"ok": true, "skipped": true})));
    }
    if !matches!(input.kind.as_str(), "review" | "fix_task" | "verify") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại sự kiện không hợp lệ",
        ));
    }
    state
        .event_log
        .append(&Event {
            ts: mutis::now(),
            plan_id: Some(input.plan_id),
            task_id: Some(input.task_id),
            provider_id: input.provider_id,
            kind: input.kind,
            task_kind: input.task_kind,
            ok: input.ok,
            duration_ms: None,
            tokens: None,
            detail: input.detail,
        })
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không thể ghi nhật ký MutiS",
            )
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
