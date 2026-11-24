use super::{super::run::Run, record::avatar_source};
use crate::core::time::now_ms;
use crate::{
    app::{AppState, JobGuard},
    chat::{self, ChatRecord},
    config::MutisConfig,
    features::{changes::Snapshot, memory::MemoryConfig},
    mutis::Event,
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::response::{
    sse::{Event as SseEvent, KeepAlive, Sse},
    IntoResponse, Response,
};
use futures_util::StreamExt;
use serde_json::json;
use std::{convert::Infallible, time::Instant};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Cancels the run when the browser drops the stream.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(super) fn status_role(
    config: &MutisConfig,
    provider_id: &str,
    is_mutis_or_plan: bool,
) -> Option<&'static str> {
    if !is_mutis_or_plan || provider_id.is_empty() {
        return None;
    }
    [
        ("lead", config.lead.provider_id.as_str()),
        ("worker", config.worker.provider_id.as_str()),
        ("checker", config.checker.provider_id.as_str()),
    ]
    .into_iter()
    .find_map(|(role, configured_id)| (configured_id == provider_id).then_some(role))
}

pub(super) async fn start(
    state: AppState,
    provider: Box<dyn providers::Provider>,
    provider_request: ProviderRequest,
    record: ChatRecord,
    cancel: CancellationToken,
    provider_cancel: CancellationToken,
    request_id: Uuid,
    sse_sender: mpsc::Sender<SseEvent>,
    sse_receiver: mpsc::Receiver<SseEvent>,
    provider_sender: mpsc::Sender<ProviderEvent>,
    provider_receiver: mpsc::Receiver<ProviderEvent>,
    mutis_config: MutisConfig,
    memory_config: MemoryConfig,
    files_before: Snapshot,
    rotated: Vec<(String, u64)>,
    token_budget: u64,
    chat_id: String,
    plan_id: Option<String>,
    task_id: Option<String>,
    task_kind: Option<String>,
) -> Response {
    let event_provider_id = provider_request.provider.id.clone();
    let provider_name = provider_request.provider.name.clone();
    let provider_kind = avatar_source(&provider_request.provider, &provider_request.model);
    let status_model = provider_request.model.clone();
    let status_effort = format!("{:?}", provider_request.effort).to_lowercase();
    let status_role = status_role(
        &mutis_config,
        &event_provider_id,
        record.mutis_level.is_some() || plan_id.is_some(),
    );
    for (key, tokens) in rotated {
        let which = match key.strip_prefix("worker:") {
            Some(worker) => format!("worker {worker}"),
            None => "chính".to_owned(),
        };
        let _ = providers::emit_text(
            &provider_sender,
            "log",
            format!("Thread {which} đã đọc {tokens} token (ngưỡng {token_budget}), mở thread mới."),
        )
        .await;
    }
    let idle_timeout_minutes = state.config.read().await.chat_idle_timeout_minutes;
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
    let event_logging_enabled = mutis_config.effective(mutis_config.event_log);
    let log_enabled = event_logging_enabled && plan_id.is_some() && task_id.is_some();
    let started = Instant::now();
    let started_at_ms = now_ms();
    let _ = sse_sender
        .send(chat::sse_event(
            "status",
            &json!({
                "provider_name": provider_name,
                "provider_kind": provider_kind,
                "model": status_model,
                "effort": status_effort,
                "role": status_role,
                "started_at_ms": started_at_ms,
            }),
        ))
        .await;
    if log_enabled {
        let _ = state.event_log.append(&Event {
            ts: crate::core::time::now_secs(),
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
    tokio::spawn(
        Run {
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
            provider_receiver,
            idle_timeout_minutes,
            memory_config,
            files_before,
        }
        .run(),
    );
    let stream_cancel = cancel.clone();
    let event_stream = async_stream::stream! {
        let _guard = CancelOnDrop(stream_cancel);
        let mut receiver = ReceiverStream::new(sse_receiver);
        while let Some(event) = receiver.next().await {
            yield Ok::<_, Infallible>(event);
        }
    };
    Sse::new(event_stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
