use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    mutis,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) async fn mutis_stats(
    State(state): State<AppState>,
    Query(query): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    let days = query
        .get("days")
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(30);
    let stats = mutis::build_stats(&state.event_log, days).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được nhật ký MutiS",
        )
    })?;
    Ok(Json(stats))
}
