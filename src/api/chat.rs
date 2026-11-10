use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard, RunningChat},
    chat::{self, ChatMessage, ChatRecord, ChatRequest, ChatStore},
    mutis::{self, Event},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::json;
use std::{
    convert::Infallible,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Serialize)]
pub(crate) struct StopResponse {
    stopped: bool,
}

pub(crate) async fn chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> ApiResult<Response> {
    if !chat::valid_id(&request.chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    let existing = state.chats.get(&request.chat_id).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể đọc lịch sử chat",
        )
    })?;
    let dev_chat = request.dev_chat || existing.as_ref().is_some_and(|chat| chat.dev_chat);
    let requested_folder = if dev_chat && state.dev_repo.is_some() {
        PathBuf::from(crate::api::dev::effective_chat_folder(
            true,
            true,
            request.folder.trim(),
            &crate::api::dev::dev_repo_path(&state)?.to_string_lossy(),
        ))
    } else {
        PathBuf::from(crate::api::dev::effective_chat_folder(
            false,
            dev_chat,
            request.folder.trim(),
            "",
        ))
    };
    let folder = requested_folder.to_string_lossy().into_owned();
    if folder.is_empty() || !PathBuf::from(&folder).is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = PathBuf::from(folder)
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Không thể giải quyết thư mục làm việc",
            )
        })?
        .to_string_lossy()
        .into_owned();
    if request.model.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Hãy chọn model"));
    }
    let effort = request.effort.as_deref().unwrap_or("default");
    let effort = chat::parse_effort(effort)
        .ok_or_else(|| ApiError::new(StatusCode::BAD_REQUEST, "Mức suy luận không hợp lệ"))?;
    if request.messages.is_empty()
        || request
            .messages
            .iter()
            .any(|message| !matches!(message.role.as_str(), "user" | "assistant" | "system"))
        || request.messages.last().map(|message| message.role.as_str()) != Some("user")
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Lịch sử chat không hợp lệ",
        ));
    }
    let provider_config = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.id == request.provider_id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    let mutis_config = state.config.read().await.mutis.clone();
    if provider_config.kind == "deepseek_agent"
        && !mutis_config.effective(mutis_config.deepseek_agent)
    {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Bật MutiS › DeepSeek làm việc trên file để dùng nguồn này",
        ));
    }
    let provider = providers::for_kind(&provider_config.kind)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Loại nhà cung cấp không hợp lệ"))?;
    let mut record = existing.unwrap_or_else(|| chat::new_record(&request));
    record.provider_id = request.provider_id.clone();
    record.model = request.model.clone();
    record.folder = folder.clone();
    record.dev_chat = dev_chat;
    record.permission = request.permission;
    record.effort = effort;
    record.messages = request
        .messages
        .iter()
        .map(|message| {
            let mut saved = message.clone();
            saved.attachments = crate::api::attachments::resolve_message_attachments(
                &state,
                &request.chat_id,
                &message.attachments,
            )?;
            Ok(saved)
        })
        .collect::<ApiResult<Vec<_>>>()?;
    if dev_chat {
        let preamble = crate::api::dev::dev_chat_preamble(&folder);
        if let Some(first) = record
            .messages
            .iter_mut()
            .find(|message| message.role == "user")
        {
            if !first.content.starts_with(&preamble) {
                first.content = format!("{preamble}\n\n{}", first.content);
            }
        }
    }
    record.messages.push(ChatMessage {
        role: "assistant".to_owned(),
        content: String::new(),
        reasoning: String::new(),
        logs: Vec::new(),
        diff: String::new(),
        error: None,
        attachments: Vec::new(),
    });
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
    let (sse_sender, sse_receiver) = mpsc::channel::<axum::response::sse::Event>(32);
    let (provider_sender, mut provider_receiver) = mpsc::channel::<ProviderEvent>(32);
    let provider_cancel = cancel.clone();
    let provider_request = ProviderRequest {
        provider: provider_config,
        model: request.model,
        folder,
        permission: request.permission,
        effort,
        messages: record
            .messages
            .iter()
            .filter(|message| message.role != "assistant")
            .cloned()
            .collect(),
        session_id: record.session_id.clone(),
        mutis: mutis_config.clone(),
    };
    let event_provider_id = provider_request.provider.id.clone();
    let provider_events = provider_sender.clone();
    tokio::spawn(async move {
        if let Err(error) = provider
            .run(provider_request, provider_sender, provider_cancel)
            .await
        {
            let _ = provider_events
                .send(ProviderEvent::text("error", error.to_string()))
                .await;
        }
    });
    let task_state = state.clone();
    let chat_id = request.chat_id;
    let plan_id = request.plan_id;
    let task_id = request.task_id;
    let task_kind = request.task_kind;
    let event_logging_enabled = mutis_config.effective(mutis_config.event_log);
    let log_enabled = event_logging_enabled && plan_id.is_some() && task_id.is_some();
    let started = Instant::now();
    if log_enabled {
        let _ = state.event_log.append(&Event {
            ts: mutis::now(),
            plan_id: plan_id.clone(),
            task_id: task_id.clone(),
            provider_id: event_provider_id.clone(),
            kind: "task_start".into(),
            task_kind: task_kind.clone(),
            ok: true,
            duration_ms: None,
            tokens: None,
            detail: None,
        });
    }
    let task_cancel = cancel.clone();
    let job_guard = JobGuard::new(state.active_jobs.clone());
    tokio::spawn(async move {
        let _job_guard = job_guard;
        let mut record = record;
        let mut last_persist = Instant::now();
        loop {
            tokio::select! {
                _ = task_cancel.cancelled() => break,
                _ = sse_sender.closed() => {
                    task_cancel.cancel();
                    break;
                }
                event = provider_receiver.recv() => {
                    let Some(event) = event else { break };
                    if event_logging_enabled && event.name == "watchdog_kill" {
                        let _ = task_state.event_log.append(&Event {
                            ts: mutis::now(),
                            plan_id: plan_id.clone(),
                            task_id: task_id.clone(),
                            provider_id: event_provider_id.clone(),
                            kind: "watchdog_kill".into(),
                            task_kind: None,
                            ok: false,
                            duration_ms: None,
                            tokens: None,
                            detail: Some("Codex watchdog".into()),
                        });
                    }
                    if event_logging_enabled && event.name == "retry" {
                        let _ = task_state.event_log.append(&Event {
                            ts: mutis::now(),
                            plan_id: plan_id.clone(),
                            task_id: task_id.clone(),
                            provider_id: event_provider_id.clone(),
                            kind: "retry".into(),
                            task_kind: None,
                            ok: true,
                            duration_ms: None,
                            tokens: None,
                            detail: event.data["text"].as_str().map(str::to_owned),
                        });
                    }
                    if event_logging_enabled && event.name == "error" {
                        let detail = event.data["text"]
                            .as_str()
                            .unwrap_or("Đã xảy ra lỗi với nguồn AI")
                            .to_owned();
                        let _ = task_state.event_log.append(&Event {
                            ts: mutis::now(),
                            plan_id: plan_id.clone(),
                            task_id: task_id.clone(),
                            provider_id: event_provider_id.clone(),
                            kind: "error".into(),
                            task_kind: task_kind.clone(),
                            ok: false,
                            duration_ms: None,
                            tokens: None,
                            detail: Some(detail),
                        });
                    }
                    if event_logging_enabled && event.data["text"].as_str().is_some_and(|text|text.starts_with("Tự tiếp tục lần ")) {
                        let _ = task_state.event_log.append(&Event {
                            ts: mutis::now(),
                            plan_id: plan_id.clone(),
                            task_id: task_id.clone(),
                            provider_id: event_provider_id.clone(),
                            kind: "resume".into(),
                            task_kind: None,
                            ok: true,
                            duration_ms: None,
                            tokens: None,
                            detail: event.data["text"].as_str().map(str::to_owned),
                        });
                    }
                    persist_event(&task_state.chats, &mut record, &event, &mut last_persist);
                    if sse_sender.send(chat::sse_event(event.name, &event.data)).await.is_err() {
                        task_cancel.cancel();
                        break;
                    }
                }
            }
        }
        let _ = chat::save_record(&task_state.chats, &mut record);
        if log_enabled {
            let token_line = record
                .messages
                .last()
                .and_then(|m| m.logs.iter().rev().find(|line| line.starts_with("Token:")));
            let tokens = token_line.and_then(|line| {
                line.split(|c: char| !c.is_ascii_digit())
                    .filter(|part| !part.is_empty())
                    .filter_map(|part| part.parse::<u64>().ok())
                    .take(2)
                    .reduce(u64::saturating_add)
            });
            let ok = record.messages.last().is_some_and(|m| m.error.is_none());
            let _ = task_state.event_log.append(&Event {
                ts: mutis::now(),
                plan_id,
                task_id,
                provider_id: event_provider_id,
                kind: "task_end".into(),
                task_kind,
                ok,
                duration_ms: Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
                tokens,
                detail: None,
            });
        }
        let done = ProviderEvent {
            name: "done",
            data: json!({ "ok": true }),
        };
        let _ = sse_sender
            .send(chat::sse_event(done.name, &done.data))
            .await;
        let mut running = task_state.running.lock().await;
        if running
            .get(&chat_id)
            .is_some_and(|entry| entry.request_id == request_id)
        {
            running.remove(&chat_id);
        }
    });
    let stream_cancel = cancel.clone();
    let event_stream = async_stream::stream! {
        let _guard = CancelOnDrop(stream_cancel);
        let mut receiver = ReceiverStream::new(sse_receiver);
        while let Some(event) = receiver.next().await {
            yield Ok::<_, Infallible>(event);
        }
    };
    Ok(Sse::new(event_stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
fn persist_event(
    store: &ChatStore,
    record: &mut ChatRecord,
    event: &ProviderEvent,
    last_persist: &mut Instant,
) {
    if event.name == "session" {
        if let Some(session_id) = event.data["session_id"]
            .as_str()
            .or_else(|| event.data["text"].as_str())
        {
            record.session_id = Some(session_id.to_owned());
            let _ = chat::save_record(store, record);
            *last_persist = Instant::now();
        }
        return;
    }
    let Some(message) = record
        .messages
        .last_mut()
        .filter(|message| message.role == "assistant")
    else {
        return;
    };
    let text = event.data["text"].as_str().unwrap_or_default();
    let force_persist = match event.name {
        "delta" => {
            message.content.push_str(text);
            false
        }
        "reasoning" => {
            message.reasoning.push_str(text);
            false
        }
        "log" => {
            message.logs.push(text.to_owned());
            true
        }
        "diff" => {
            message.diff = text.to_owned();
            true
        }
        "error" => {
            message.error = Some(text.to_owned());
            true
        }
        _ => return,
    };
    if force_persist || last_persist.elapsed() >= Duration::from_secs(1) {
        let _ = chat::save_record(store, record);
        *last_persist = Instant::now();
    }
}

struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
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
