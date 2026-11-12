//! The event loop of one chat run: forwards provider events to the browser, persists them and finishes the chat.
use super::events::{idle_timeout_reached, persist_event, should_emit_empty_reply};
use crate::{
    app::{AppState, JobGuard},
    chat::{self, ChatRecord},
    features::{changes::Snapshot, memory::MemoryConfig},
    mutis::{self, Event},
    providers::ProviderEvent,
};
use serde_json::json;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub(super) struct Run {
    pub job_guard: JobGuard,
    pub record: ChatRecord,
    pub task_state: AppState,
    pub chat_id: String,
    pub request_id: Uuid,
    pub plan_id: Option<String>,
    pub task_id: Option<String>,
    pub task_kind: Option<String>,
    pub event_provider_id: String,
    pub event_logging_enabled: bool,
    pub log_enabled: bool,
    pub started: Instant,
    pub task_cancel: CancellationToken,
    pub sse_sender: mpsc::Sender<axum::response::sse::Event>,
    pub provider_receiver: mpsc::Receiver<ProviderEvent>,
    pub idle_timeout_minutes: u32,
    pub memory_config: MemoryConfig,
    pub files_before: Snapshot,
}

impl Run {
    pub(super) async fn run(self) {
        let Run {
            job_guard, record, task_state, chat_id, request_id, plan_id, task_id, task_kind,
            event_provider_id, event_logging_enabled, log_enabled, started, task_cancel,
            sse_sender, mut provider_receiver, idle_timeout_minutes, memory_config, files_before,
        } = self;
        let _job_guard = job_guard;
        let mut record = record;
        let mut last_persist = Instant::now();
        let mut last_event_at = Instant::now();
        let mut last_activity = String::new();
        let mut heartbeat = tokio::time::interval(Duration::from_secs(3));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut received_delta = false;
        let mut received_error = false;
        loop {
            tokio::select! {
                _ = task_cancel.cancelled() => break,
                _ = sse_sender.closed() => {
                    task_cancel.cancel();
                    break;
                }
                _ = heartbeat.tick() => {
                    let idle_s = last_event_at.elapsed().as_secs();
                    if idle_timeout_reached(last_event_at.elapsed(), idle_timeout_minutes) {
                        task_cancel.cancel();
                        let message = format!("Không có phản hồi sau {idle_timeout_minutes} phút — đã dừng");
                        let event = ProviderEvent::text("error", message);
                        persist_event(&task_state.chats, &mut record, &event, &mut last_persist);
                        received_error = true;
                        let _ = sse_sender.send(chat::sse_event("error", &event.data)).await;
                        if event_logging_enabled {
                            let _ = task_state.event_log.append(&Event {
                                ts: mutis::now(), plan_id: plan_id.clone(), task_id: task_id.clone(),
                                provider_id: event_provider_id.clone(), kind: "watchdog_kill".into(),
                                task_kind: task_kind.clone(), ok: false, duration_ms: None, tokens: None,
                                detail: Some(format!("Chat idle timeout: {idle_timeout_minutes} minutes")),
                            });
                        }
                        break;
                    }
                    let _ = sse_sender.send(chat::sse_event("heartbeat", &json!({
                        "elapsed_s": started.elapsed().as_secs(),
                        "idle_s": idle_s,
                        "last_activity": last_activity,
                    }))).await;
                }
                event = provider_receiver.recv() => {
                    let Some(event) = event else { break };
                    last_event_at = Instant::now();
                    if event.name == "delta" { received_delta = true; }
                    if event.name == "error" { received_error = true; }
                    if event.name == "log" {
                        last_activity = event.data["text"].as_str().unwrap_or_default().chars().take(100).collect();
                    }
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
                            detail: Some("Provider watchdog".into()),
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
                    if event.name == "done" { continue; }
                    persist_event(&task_state.chats, &mut record, &event, &mut last_persist);
                    if sse_sender.send(chat::sse_event(event.name, &event.data)).await.is_err() {
                        task_cancel.cancel();
                        break;
                    }
                }
            }
        }
        if should_emit_empty_reply(received_delta, received_error, task_cancel.is_cancelled()) {
            let event = ProviderEvent::text(
                "error",
                "AI kết thúc mà không trả lời — có thể bị ngắt giữa chừng. Hãy gửi lại.",
            );
            persist_event(&task_state.chats, &mut record, &event, &mut last_persist);
            let _ = sse_sender.send(chat::sse_event("error", &event.data)).await;
        }
        let files_after = crate::features::changes::snapshot(std::path::Path::new(&record.folder)).await;
        let files = crate::features::changes::changes(&files_before, &files_after);
        if !files.is_empty() {
            let _ = sse_sender
                .send(chat::sse_event("files", &json!({ "files": files })))
                .await;
            if let Some(message) = record.messages.last_mut() {
                message.files = files.clone();
            }
        }
        if let Some(message) = record.messages.last_mut() {
            let (clean, block) = crate::features::memory::strip_blocks(&message.content);
            message.content = clean;
            if memory_config.enabled {
                if let Some(block) = block.filter(|b| b.trim() != "none") {
                    match serde_json::from_str::<crate::features::memory::Update>(&block) {
                        Ok(update) => {
                            if let Ok(store) = crate::features::memory::Store::open(
                                &task_state.config_path,
                                &record.folder,
                                &memory_config,
                            ) {
                                let paths = files.iter().map(|f| f.path.clone()).collect();
                                if let Ok(change) = store.merge(update, &chat_id, paths) {
                                    let _ = sse_sender
                                        .send(chat::sse_event("memory", &json!(change)))
                                        .await;
                                }
                            }
                        }
                        Err(_) => {
                            let text = "Bộ nhớ: khối <memory> không hợp lệ";
                            message.logs.push(text.into());
                            let _ = sse_sender
                                .send(chat::sse_event("log", &json!({"text":text})))
                                .await;
                        }
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
    }
}
