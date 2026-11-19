use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRecord},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct StopResponse {
    stopped: bool,
}

pub(crate) async fn stop_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<StopResponse>> {
    if !chat::valid_id(&chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    let running = state.running.lock().await;
    let stopped = running.get(&chat_id).is_some_and(|request| {
        request.cancel.cancel();
        true
    });
    Ok(Json(StopResponse { stopped }))
}
#[derive(Serialize)]
pub(crate) struct CompactResponse {
    compacted: usize,
    failed: Vec<String>,
}

/// The "Nén ngữ cảnh" button: compacts every Codex thread of the chat (lead, workers, tester). Never while the chat is running.
pub(crate) async fn compact_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<CompactResponse>> {
    if !chat::valid_id(&chat_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "chat_id không hợp lệ"));
    }
    if state.running.lock().await.contains_key(&chat_id) {
        return Err(ApiError::new(StatusCode::CONFLICT, "Chat đang chạy; đợi xong rồi nén ngữ cảnh"));
    }
    let record = state
        .chats
        .get(&chat_id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể đọc lịch sử chat"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy chat"))?;
    let (providers, settings) = {
        let config = state.config.read().await;
        (config.providers.clone(), config.codex.clone())
    };
    let mut threads: Vec<&String> = record.session_id.iter().chain(&record.checker_session_id).chain(record.worker_sessions.values()).collect();
    threads.sort();
    threads.dedup();
    let (mut compacted, mut failed) = (0, Vec::new());
    for provider in providers.iter().filter(|p| p.kind == "codex_cli") {
        let prefix = crate::providers::codex::session_prefix(&settings, provider, &record.folder);
        let ids: Vec<&str> = threads.iter().filter_map(|id| id.strip_prefix(prefix.as_str())).collect();
        if ids.is_empty() {
            continue;
        }
        let Some(executable) = crate::providers::cli_support::resolve_executable("codex", Some(&provider.executable)) else {
            failed.push("Chưa tìm thấy Codex CLI trên máy".to_owned());
            continue;
        };
        let home = crate::providers::codex::home(&state.config_path, provider).map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;
        for id in ids {
            match crate::providers::codex::compact::compact_saved(&executable, &home, std::path::Path::new(&record.folder), id).await {
                Ok(()) => compacted += 1,
                Err(error) => failed.push(error.to_string()),
            }
        }
    }
    Ok(Json(CompactResponse { compacted, failed }))
}
pub(crate) async fn list_chats(
    State(state): State<AppState>,
) -> ApiResult<Json<Vec<chat::ChatSummary>>> {
    state.chats.list().map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể liệt kê lịch sử chat",
        )
    })
}
pub(crate) async fn get_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<ChatRecord>> {
    if !chat::valid_id(&chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    state
        .chats
        .get(&chat_id)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không thể đọc lịch sử chat",
            )
        })?
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy chat"))
}
/// Read-only: the recorded token snapshots of one chat (latest 500), never creates or repairs files.
pub(crate) async fn token_history(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Vec<crate::features::token_usage::TokenSnapshot>>> {
    if !chat::valid_id(&chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    state.chats.token_history(&chat_id).map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể đọc lịch sử token của chat",
        )
    })
}
pub(crate) async fn delete_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<StatusCode> {
    if !chat::valid_id(&chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    let _ = state
        .chats
        .delete(&chat_id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể xoá chat"))?;
    if let Some(running) = state.running.lock().await.get(&chat_id) {
        running.cancel.cancel();
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::RunningChat;

    fn state() -> (AppState, String) {
        let root = std::env::temp_dir().join(format!("mutis-compact-api-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let state = AppState::new(crate::config::Config::seeded(), root.join("config.json"), root.join("chats"), None, 0);
        let id = uuid::Uuid::new_v4().to_string();
        let record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": id, "title": "t", "provider_id": "codex-cli", "model": "m", "folder": root.to_string_lossy(), "messages": [], "updated_at": 1
        })).unwrap();
        state.chats.save(&record).unwrap();
        (state, id)
    }

    #[tokio::test]
    async fn compact_refuses_bad_ids_unknown_chats_and_running_chats() {
        let (state, id) = state();
        let status = |result: ApiResult<Json<CompactResponse>>| result.err().map(|e| e.status);
        assert_eq!(status(compact_chat(State(state.clone()), Path("not-a-uuid".into())).await), Some(StatusCode::BAD_REQUEST));
        assert_eq!(status(compact_chat(State(state.clone()), Path(uuid::Uuid::new_v4().to_string())).await), Some(StatusCode::NOT_FOUND));
        state.running.lock().await.insert(id.clone(), RunningChat { request_id: uuid::Uuid::new_v4(), cancel: tokio_util::sync::CancellationToken::new() });
        assert_eq!(status(compact_chat(State(state), Path(id)).await), Some(StatusCode::CONFLICT));
    }

    #[tokio::test]
    async fn a_chat_without_codex_threads_compacts_nothing() {
        let (state, id) = state();
        let Json(result) = compact_chat(State(state), Path(id)).await.ok().unwrap();
        assert_eq!((result.compacted, result.failed.len()), (0, 0));
    }
}
