//! The event loop of one chat run: forwards provider events to the browser, persists them and finishes the chat.
mod finish;
mod log;

use self::finish::finish;
use self::log::event_builder;
use super::events::{idle_timeout_reached, persist_event};
use crate::{
    app::{AppState, JobGuard},
    chat::{self, ChatRecord},
    features::{changes::Snapshot, memory::MemoryConfig},
    providers::ProviderEvent,
};
use serde_json::{json, Value};
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
            job_guard,
            record,
            task_state,
            chat_id,
            request_id,
            plan_id,
            task_id,
            task_kind,
            event_provider_id,
            event_logging_enabled,
            log_enabled,
            started,
            task_cancel,
            sse_sender,
            mut provider_receiver,
            idle_timeout_minutes,
            memory_config,
            files_before,
        } = self;
        let make_event = event_builder(&plan_id, &task_id, &event_provider_id);
        let _job_guard = job_guard;
        let mut record = record;
        let first_turn = record.messages.len().saturating_sub(1);
        let mut delegated = false;
        let mut task_snapshot: Option<Vec<Value>> = None;
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
                            let _ = task_state.event_log.append(&make_event(
                                "watchdog_kill",
                                task_kind.clone(),
                                false,
                                None,
                                None,
                                Some(format!("Chat idle timeout: {idle_timeout_minutes} minutes")),
                            ));
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
                    if event.name == "turn" {
                        delegated = true;
                        last_activity.clear();
                        received_delta = false;
                        received_error = false;
                    }
                    if event.name == "delta" { received_delta = true; }
                    if event.name == "error" { received_error = true; }
                    if event.name == "tasks" {
                        task_snapshot = event.data["tasks"].as_array().cloned();
                    }
                    if event.name == "log" {
                        last_activity = event.data["text"].as_str().unwrap_or_default().chars().take(100).collect();
                    }
                    if event_logging_enabled && event.name == "watchdog_kill" {
                        let _ = task_state.event_log.append(&make_event(
                            "watchdog_kill",
                            None,
                            false,
                            None,
                            None,
                            Some("Provider watchdog".into()),
                        ));
                    }
                    if event_logging_enabled && event.name == "retry" {
                        let _ = task_state.event_log.append(&make_event(
                            "retry",
                            None,
                            true,
                            None,
                            None,
                            event.data["text"].as_str().map(str::to_owned),
                        ));
                    }
                    if event_logging_enabled && event.name == "error" {
                        let detail = event.data["text"]
                            .as_str()
                            .unwrap_or("Đã xảy ra lỗi với nguồn AI")
                            .to_owned();
                        let _ = task_state.event_log.append(&make_event(
                            "error",
                            task_kind.clone(),
                            false,
                            None,
                            None,
                            Some(detail),
                        ));
                    }
                    if event_logging_enabled && event.data["text"].as_str().is_some_and(|text|text.starts_with("Tự tiếp tục lần ")) {
                        let _ = task_state.event_log.append(&make_event(
                            "resume",
                            None,
                            true,
                            None,
                            None,
                            event.data["text"].as_str().map(str::to_owned),
                        ));
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
        finish(
            &mut record,
            &sse_sender,
            &task_state,
            &task_cancel,
            received_delta,
            received_error,
            &mut last_persist,
            delegated,
            first_turn,
            &files_before,
            &memory_config,
            &chat_id,
            request_id,
            task_snapshot,
            log_enabled,
            started,
            task_kind,
            make_event,
        )
        .await;
    }
}
