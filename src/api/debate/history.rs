use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat,
    features::debate::{self, Debate},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;

pub(crate) async fn list_debates(State(state): State<AppState>) -> ApiResult<Json<Vec<Debate>>> {
    state.debates.list().map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được lịch sử tranh luận",
        )
    })
}

pub(crate) async fn get_debate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let debate = state
        .debates
        .get(&id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "id tranh luận không hợp lệ"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy tranh luận"))?;
    let plan_request = debate::plan_request(&debate);
    let mut value = json!(debate);
    value["plan_request"] = json!(plan_request);
    Ok(Json(value))
}

#[derive(Deserialize)]
pub(crate) struct ChatLink {
    chat_id: String,
}

/// Adopt a legacy record once, without starting another debate or provider run.
pub(crate) async fn attach_chat(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ChatLink>,
) -> ApiResult<Json<Debate>> {
    if !chat::valid_id(&id) || !chat::valid_id(&input.chat_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "id không hợp lệ"));
    }
    let operation = || -> anyhow::Result<Debate> {
        let mut record = state
            .debates
            .get(&id)?
            .ok_or_else(|| anyhow::anyhow!("Không tìm thấy tranh luận"))?;
        if record.chat_id.is_none() {
            record.chat_id = Some(input.chat_id.clone());
            record.save_chat(&state.chats, &input.chat_id, true)?;
            record.save_chat(&state.chats, &input.chat_id, false)?;
            state.debates.save(&record)?;
        }
        Ok(record)
    };
    operation()
        .map(Json)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
