use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
    mutis::{self, DiagnosisError, DiagnosisRun},
    providers::ProviderEvent,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde_json::{json, Value};
use std::convert::Infallible;
use tokio::sync::mpsc;

pub(crate) async fn mutis_diagnoses(State(state): State<AppState>) -> ApiResult<Json<Vec<Value>>> {
    Ok(Json(mutis::list_diagnoses(&state)))
}

pub(crate) async fn mutis_diagnose(State(state): State<AppState>) -> ApiResult<Response> {
    let _job_guard = JobGuard::new(state.active_jobs.clone());
    let run = mutis::start_diagnosis(&state).await.map_err(|error| {
        let (status, message) = match error {
            DiagnosisError::ReadEvents => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được nhật ký")
            }
            DiagnosisError::ReadStats => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không đọc được nhật ký MutiS",
            ),
            DiagnosisError::Save(message) => (StatusCode::INTERNAL_SERVER_ERROR, message),
            DiagnosisError::NoConnectedLead(message) => (StatusCode::SERVICE_UNAVAILABLE, message),
            DiagnosisError::InvalidProviderKind => {
                (StatusCode::BAD_REQUEST, "Loại nguồn AI không hợp lệ")
            }
        };
        ApiError::new(status, message)
    })?;
    match run {
        DiagnosisRun::Complete(diagnosis) => {
            let stream = async_stream::stream! {
                yield Ok::<_, Infallible>(sse("diagnosis", diagnosis));
            };
            Ok(Sse::new(stream)
                .keep_alive(KeepAlive::default())
                .into_response())
        }
        DiagnosisRun::Running {
            receiver,
            provider_id,
            ts,
        } => Ok(diagnosis_stream(state, receiver, provider_id, ts)),
    }
}

fn diagnosis_stream(
    state: AppState,
    mut receiver: mpsc::Receiver<ProviderEvent>,
    provider_id: String,
    ts: u64,
) -> Response {
    let stream = async_stream::stream! {
        let mut raw = String::new();
        let mut failed = false;
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => raw.push_str(event.data["text"].as_str().unwrap_or_default()),
                "log" => yield Ok::<_, Infallible>(sse("log", event.data)),
                "error" => {
                    failed = true;
                    yield Ok(sse("error", event.data));
                }
                _ => {}
            }
        }
        if !failed {
            match mutis::finish_diagnosis(&state, ts, provider_id, &raw) {
                Ok(diagnosis) => yield Ok(sse("diagnosis", diagnosis)),
                Err(message) => yield Ok(sse("error", json!({"message": message}))),
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

fn sse(name: &str, data: Value) -> SseEvent {
    SseEvent::default()
        .event(name)
        .json_data(data)
        .unwrap_or_default()
}
