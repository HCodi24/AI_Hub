use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, RunningChat},
    chat::{self, ChatMessage, ChatRecord, ChatRequest},
    config::ProviderConfig,
    providers,
};
use axum::http::StatusCode;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// What the chat avatar is chosen by: the source kind, except DeepSeek models (by model, source id or name), which get their own.
pub(super) fn avatar_source(provider: &crate::config::ProviderConfig, model: &str) -> String {
    if provider.is_deepseek(model) {
        "deepseek".to_owned()
    } else {
        provider.kind.clone()
    }
}

pub(super) async fn save_and_register(
    state: &AppState,
    request: &ChatRequest,
    existing: Option<ChatRecord>,
    provider_config: &ProviderConfig,
    mutis_level: Option<&str>,
    folder: &str,
    plain_chat: bool,
    dev_chat: bool,
    file_tools: chat::FileTools,
    effort: chat::Effort,
) -> ApiResult<(ChatRecord, CancellationToken, Uuid)> {
    // The thread belongs to the source and to the mode (MutiS lead or a single model), so the same Codex in the two modes never shares one.
    let thread = chat::thread_key(&request.provider_id, mutis_level.is_some());
    let switched_source = existing.as_ref().is_some_and(|r| r.thread_key() != thread);
    let mut record = existing.unwrap_or_else(|| chat::new_record(request));
    if switched_source {
        record.switch_source(&thread);
    }
    if provider_config.kind == "codex_cli" {
        let settings = state.config.read().await.codex.clone();
        let prefix = providers::codex::session_prefix(&settings, provider_config, folder);
        if !record
            .session_id
            .as_deref()
            .is_some_and(|id| id.starts_with(&prefix))
        {
            record.session_id = None;
        }
        // The checker is Codex too, so its thread belongs to the same Codex home as the coordinator's.
        if !record
            .checker_session_id
            .as_deref()
            .is_some_and(|id| id.starts_with(&prefix))
        {
            record.checker_session_id = None;
        }
    }
    record.provider_id = request.provider_id.clone();
    record.mutis_level = mutis_level.map(str::to_owned);
    record.model = request.model.clone();
    record.folder = if plain_chat {
        String::new()
    } else {
        folder.to_owned()
    };
    record.dev_chat = dev_chat;
    record.permission = request.permission;
    record.file_tools = file_tools;
    record.effort = effort;
    record.messages = request
        .messages
        .iter()
        .map(|message| {
            let mut saved = message.clone();
            if saved.role == "assistant" {
                saved.content = crate::features::memory::strip_blocks(&saved.content).0;
            }
            saved.attachments = crate::api::attachments::resolve_message_attachments(
                state,
                &request.chat_id,
                &message.attachments,
            )?;
            Ok(saved)
        })
        .collect::<ApiResult<Vec<_>>>()?;
    record.messages.push(ChatMessage::assistant(
        &avatar_source(provider_config, &request.model),
        request.model.clone(),
    ));
    chat::save_record(&state.chats, &mut record).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể lưu lịch sử chat",
        )
    })?;
    let cancel = CancellationToken::new();
    let request_id = Uuid::new_v4();
    {
        let mut running = state.running.lock().await;
        if let Some(previous) = running.insert(
            request.chat_id.clone(),
            RunningChat {
                request_id,
                cancel: cancel.clone(),
            },
        ) {
            previous.cancel.cancel();
        }
    }
    Ok((record, cancel, request_id))
}
