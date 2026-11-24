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

fn check_chat_id(chat_id: &str, message: &'static str) -> ApiResult<()> {
    if !chat::valid_id(chat_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, message));
    }
    Ok(())
}

#[derive(Serialize)]
pub(crate) struct StopResponse {
    stopped: bool,
}

pub(crate) async fn stop_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<StopResponse>> {
    check_chat_id(&chat_id, "chat_id phải là UUID hợp lệ")?;
    let running = state.running.lock().await;
    let stopped = if let Some(request) = running.get(&chat_id) {
        request.cancel.cancel();
        true
    } else {
        false
    };
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
    check_chat_id(&chat_id, "chat_id không hợp lệ")?;
    if state.running.lock().await.contains_key(&chat_id) {
        return Err(ApiError::new(StatusCode::CONFLICT, "Chat đang chạy; đợi xong rồi nén ngữ cảnh"));
    }
    let record = state
        .chats
        .get(&chat_id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể đọc lịch sử chat"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy chat"))?;
    let (providers, settings, antigravity) = {
        let config = state.config.read().await;
        (config.providers.clone(), config.codex.clone(), config.antigravity.clone())
    };
    let mut threads: Vec<&String> = record
        .session_id
        .iter()
        .chain(&record.checker_session_id)
        .chain(record.worker_sessions.values())
        .collect();
    threads.sort();
    threads.dedup();
    let (mut compacted, mut failed) = (0, Vec::new());
    for provider in providers.iter().filter(|p| p.kind == "codex_cli") {
        let prefix = crate::providers::codex::session_prefix(&settings, provider, &record.folder);
        let ids: Vec<&str> = threads.iter().filter_map(|id| id.strip_prefix(prefix.as_str())).collect();
        if ids.is_empty() {
            continue;
        }
        let Some(executable) =
            crate::providers::cli_support::resolve_executable("codex", Some(&provider.executable))
        else {
            failed.push("Chưa tìm thấy Codex CLI trên máy".to_owned());
            continue;
        };
        let home = crate::providers::codex::home(&state.config_path, provider)
            .map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;
        for id in ids {
            match crate::providers::codex::compact::compact_saved(
                &executable,
                &home,
                std::path::Path::new(&record.folder),
                id,
            )
            .await
            {
                Ok(()) => compacted += 1,
                Err(error) => failed.push(error.to_string()),
            }
        }
    }
    // Claude sessions (the lead): the same button, Claude's own `/compact`.
    for provider in providers.iter().filter(|p| p.kind == "claude_cli") {
        let (Ok(settings), Ok(home)) = (
            crate::providers::claude::environment::settings(),
            crate::providers::claude::environment::home(&state.config_path, provider),
        ) else {
            continue;
        };
        let prefix = crate::providers::claude::environment::session_prefix(&settings, provider, &record.folder);
        for id in threads.iter().filter_map(|id| id.strip_prefix(prefix.as_str())) {
            match crate::providers::claude_cli::compact_session(
                provider,
                &home,
                &record.folder,
                id,
                "",
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            {
                Ok(()) => compacted += 1,
                Err(error) => failed.push(error.to_string()),
            }
        }
    }
    // Antigravity conversations: agy's own `/compact`.
    for provider in providers.iter().filter(|p| p.kind == "antigravity_cli") {
        let prefix = crate::providers::antigravity_cli::session_prefix(provider, &record.folder, &antigravity);
        let Ok(home) = crate::providers::antigravity_cli::clean_home(&provider.id) else { continue };
        for id in threads.iter().filter_map(|id| id.strip_prefix(prefix.as_str())) {
            match crate::providers::antigravity_cli::compact_session(
                provider,
                &home,
                &record.folder,
                id,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            {
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
    check_chat_id(&chat_id, "chat_id không hợp lệ")?;
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
    check_chat_id(&chat_id, "chat_id không hợp lệ")?;
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
    check_chat_id(&chat_id, "chat_id không hợp lệ")?;
    {
        let running = state.running.lock().await;
        if let Some(request) = running.get(&chat_id) {
            request.cancel.cancel();
        }
    }
    // Ignore whether the chat existed: deletion is idempotent.
    state
        .chats
        .delete(&chat_id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể xoá chat"))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
pub(crate) struct Approval {
    allow: bool,
}

/// The user's answer to a `permission_request` of a running chat.
pub(crate) async fn answer_approval(Path(id): Path<String>, Json(body): Json<Approval>) -> ApiResult<StatusCode> {
    if crate::providers::approvals::answer(&id, body.allow) {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::new(StatusCode::NOT_FOUND, "Không có yêu cầu nào đang chờ (đã trả lời hoặc hết hạn)"))
    }
}

#[cfg(test)]
mod tests;
