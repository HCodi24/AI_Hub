use super::mutis_stats;
use crate::{
    api::providers::cached_provider_test,
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
    chat::{self, ChatMessage},
    config::{Config, ProviderConfig},
    mutis::{self, Event},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde_json::{json, Value};
use std::{collections::HashMap, convert::Infallible, path::PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) async fn mutis_diagnoses(State(state): State<AppState>) -> ApiResult<Json<Vec<Value>>> {
    let directory = diagnoses_dir(&state);
    let mut items: Vec<Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            if let Ok(data) = std::fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_json::from_str(&data) {
                    items.push(value);
                }
            }
        }
    }
    items.sort_by_key(|v| std::cmp::Reverse(v["ts"].as_u64().unwrap_or(0)));
    Ok(Json(items))
}

pub(crate) async fn mutis_diagnose(State(state): State<AppState>) -> ApiResult<Response> {
    let _job_guard = JobGuard::new(state.active_jobs.clone());
    let all_events = state
        .event_log
        .recent(2000)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được nhật ký"))?;
    let ts = mutis::now();
    let insufficient = all_events.len() < 5;
    let config_snapshot = state.config.read().await.clone();
    let failures = failure_events(&all_events);
    let stats = mutis_stats(
        State(state.clone()),
        Query(HashMap::from([("days".to_owned(), "30".to_owned())])),
    )
    .await?
    .0;
    if insufficient {
        return insufficient_data(&state, ts);
    }
    let (provider_config, model, effort) = resolve_connected_lead(&state, &config_snapshot).await?;
    let provider = providers::for_kind(&provider_config.kind)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Loại nguồn AI không hợp lệ"))?;
    let settings = serde_json::to_value(&config_snapshot.mutis).unwrap_or(Value::Null);
    let prompt = diagnosis_prompt(&settings, &stats, &failures);
    let request = ProviderRequest {
        provider: provider_config.clone(),
        model,
        folder: config_snapshot.default_folder.clone(),
        permission: chat::Permission::Read,
        effort,
        messages: vec![ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "user".into(),
            content: prompt,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: String::new(),
            model: String::new(),
            error: None,
        }],
        session_id: None,
        mutis: config_snapshot.mutis.clone(),
        mcp: Vec::new(),
        write_scope: Vec::new(),
        checker_session_id: None,
    };
    let (sender, receiver) = mpsc::channel::<ProviderEvent>(32);
    tokio::spawn(async move {
        if let Err(error) = provider
            .run(request, sender.clone(), CancellationToken::new())
            .await
        {
            let _ = sender
                .send(ProviderEvent::text("error", error.to_string()))
                .await;
        }
    });
    Ok(diagnosis_stream(state, receiver, provider_config.id, ts))
}

fn insufficient_data(state: &AppState, ts: u64) -> ApiResult<Response> {
    let diagnosis = json!({"ts":ts,"health":"chưa đủ dữ liệu","findings":[],"summary":"Chưa đủ dữ liệu để chẩn đoán"});
    save_diagnosis(state, &diagnosis)?;
    let _ = state.event_log.append(&Event {
        ts,
        plan_id: None,
        task_id: None,
        provider_id: String::new(),
        kind: "diagnosis".into(),
        task_kind: None,
        ok: true,
        duration_ms: None,
        tokens: None,
        detail: Some(diagnosis["summary"].as_str().unwrap_or_default().into()),
    });
    let stream = async_stream::stream! {
        yield Ok::<_, Infallible>(sse("diagnosis", diagnosis));
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
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
            match crate::core::text::extract_json(&raw).and_then(|text| serde_json::from_str::<Value>(text).ok()).and_then(|value| mutis::validate_diagnosis(value).ok()) {
                Some(value) => {
                    let diagnosis = json!({"ts":ts,"health":value["health"],"findings":value["findings"],"summary":value["summary"]});
                    if let Err(error) = save_diagnosis(&state, &diagnosis) {
                        yield Ok(sse("error", json!({"message": error.message})));
                    }
                    else {
                        let _ = state.event_log.append(&Event {
                            ts,
                            plan_id: None,
                            task_id: None,
                            provider_id: provider_id.clone(),
                            kind: "diagnosis".into(),
                            task_kind: None,
                            ok: true,
                            duration_ms: None,
                            tokens: None,
                            detail: Some(
                                diagnosis["summary"]
                                    .as_str()
                                    .unwrap_or_default()
                                    .into(),
                            ),
                        });
                        yield Ok(sse("diagnosis", diagnosis));
                    }
                }
                None => yield Ok(sse(
                    "error",
                    json!({"message":"Không phân tích được kết quả chẩn đoán JSON"}),
                )),
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

pub(super) fn failure_events(all: &[Event]) -> Vec<Event> {
    let mut failure_events = all
        .iter()
        .filter(|event| {
            matches!(
                event.kind.as_str(),
                "retry" | "watchdog_kill" | "resume" | "fix_task" | "error"
            ) || (event.kind == "verify" && !event.ok)
                || (event.kind == "review" && !event.ok)
                || event
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.to_lowercase().contains("empty deepseek"))
        })
        .take(150)
        .cloned()
        .collect::<Vec<_>>();
    for event in &mut failure_events {
        if let Some(detail) = &mut event.detail {
            *detail = detail.chars().take(300).collect();
        }
    }
    failure_events
}

pub(super) fn diagnosis_prompt(settings: &Value, stats: &Value, failures: &[Event]) -> String {
    format!(
        concat!(
            "You are the read-only MutiS reviewer. Analyze the provided settings, stats ",
            "and recent failure events. Return ONLY JSON matching this contract exactly: ",
            "{{\"health\":\"tốt|trung bình|kém\",\"findings\":[",
            "{{\"problem\":\"\",\"evidence\":\"\",\"cause\":\"\",",
            "\"severity\":\"high|medium|low\",\"suggestion\":\"\",",
            "\"setting_change\":{{\"path\":\"mutis.<field> or ",
            "mutis.roles.<lead|big|small>.<provider_id|model|effort> or ",
            "mutis.routing[i].role\",\"value\":null}} or null}}],",
            "\"summary\":\"\"}}. Do not include unknown fields or propose secrets, ",
            "provider keys, paths, verify commands or worker rules.\n",
            "SETTINGS (no provider keys):\n{}\nSTATS:\n{}\n",
            "LAST FAILURE EVENTS (max 150, each detail max 300 chars):\n{}"
        ),
        settings,
        stats,
        serde_json::to_string(failures).unwrap_or_default()
    )
}

fn diagnoses_dir(state: &AppState) -> PathBuf {
    state
        .config_path
        .parent()
        .unwrap()
        .join("mutis")
        .join("diagnoses")
}

fn save_diagnosis(state: &AppState, diagnosis: &Value) -> ApiResult<()> {
    let dir = diagnoses_dir(state);
    std::fs::create_dir_all(&dir).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không lưu được chẩn đoán",
        )
    })?;
    std::fs::write(
        dir.join(format!(
            "{}.json",
            diagnosis["ts"].as_u64().unwrap_or_default()
        )),
        serde_json::to_vec_pretty(diagnosis).unwrap(),
    )
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không lưu được chẩn đoán",
        )
    })
}

async fn resolve_connected_lead(
    state: &AppState,
    config: &Config,
) -> ApiResult<(ProviderConfig, String, chat::Effort)> {
    let role = &config.mutis.roles.lead;
    let mut candidates = Vec::new();
    if let Some(provider) = config.providers.iter().find(|p| p.id == role.provider_id) {
        candidates.push(provider.clone());
    }
    for kind in ["claude_cli", "anthropic", "codex_cli"] {
        for provider in config.providers.iter().filter(|p| p.kind == kind) {
            if !candidates
                .iter()
                .any(|candidate| candidate.id == provider.id)
            {
                candidates.push(provider.clone());
            }
        }
    }
    for provider in candidates {
        if cached_provider_test(state, &provider.id)
            .await
            .is_ok_and(|status| status.status == "connected")
        {
            let model = if role.provider_id == provider.id && !role.model.is_empty() {
                role.model.clone()
            } else {
                provider.models.first().cloned().unwrap_or_default()
            };
            if model.is_empty() {
                continue;
            }
            let effort = chat::parse_effort(if role.provider_id == provider.id {
                &role.effort
            } else {
                "default"
            })
            .unwrap_or_default();
            return Ok((provider, model, effort));
        }
    }
    Err(ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "Không có nguồn đang kết nối để chẩn đoán",
    ))
}
