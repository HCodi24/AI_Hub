use crate::{
    api::providers::{cached_provider_test, ConnectionTest},
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
    chat::{self, ChatMessage},
    config::{self, Config, ProviderConfig},
    mutis::{self, Event},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, convert::Infallible};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct PlanEventInput {
    plan_id: String,
    task_id: String,
    provider_id: String,
    kind: String,
    #[serde(default)]
    task_kind: Option<String>,
    ok: bool,
    detail: Option<String>,
}
pub(crate) async fn record_mutis_event(
    State(state): State<AppState>,
    Json(input): Json<PlanEventInput>,
) -> ApiResult<Json<Value>> {
    let config = state.config.read().await;
    if !config.mutis.effective(config.mutis.event_log) {
        return Ok(Json(json!({"ok": true, "skipped": true})));
    }
    if !matches!(input.kind.as_str(), "review" | "fix_task" | "verify") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại sự kiện không hợp lệ",
        ));
    }
    state
        .event_log
        .append(&Event {
            ts: mutis::now(),
            plan_id: Some(input.plan_id),
            task_id: Some(input.task_id),
            provider_id: input.provider_id,
            kind: input.kind,
            task_kind: input.task_kind,
            ok: input.ok,
            duration_ms: None,
            tokens: None,
            detail: input.detail,
        })
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không thể ghi nhật ký MutiS",
            )
        })?;
    Ok(Json(json!({"ok": true})))
}

pub(crate) async fn get_mutis(State(state): State<AppState>) -> Json<config::MutisConfig> {
    Json(state.config.read().await.mutis.clone())
}
#[derive(Serialize)]
pub(crate) struct ResolvedRole {
    configured_provider: Option<ProviderConfig>,
    live_status: Option<ConnectionTest>,
    used_provider: Option<ProviderConfig>,
}
pub(crate) async fn resolve_mutis_roles(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let cfg = state.config.read().await.clone();
    let mut output = serde_json::Map::new();
    for (name, role, fallback) in [
        (
            "lead",
            &cfg.mutis.roles.lead,
            ["claude_cli", "anthropic", "codex_cli"].as_slice(),
        ),
        (
            "big",
            &cfg.mutis.roles.big,
            ["codex_cli", "claude_cli", "anthropic"].as_slice(),
        ),
        (
            "small",
            &cfg.mutis.roles.small,
            ["codex_cli", "claude_cli"].as_slice(),
        ),
    ] {
        // The removed legacy agent id still resolves to the agent-capable Deploy source.
        let configured =
            crate::config::resolve_provider_id(&cfg.providers, &role.provider_id).cloned();
        let mut candidates = Vec::new();
        if let Some(provider) = &configured {
            candidates.push(provider.clone());
        }
        if name == "small" {
            for provider in cfg
                .providers
                .iter()
                .filter(|p| p.kind == "openai_compat" && p.agent_capable)
            {
                if !candidates.iter().any(|x| x.id == provider.id) {
                    candidates.push(provider.clone());
                }
            }
        }
        for kind in fallback {
            for provider in cfg.providers.iter().filter(|p| p.kind == *kind) {
                if !candidates.iter().any(|x| x.id == provider.id) {
                    candidates.push(provider.clone());
                }
            }
        }
        let results = join_all(
            candidates
                .iter()
                .map(|p| cached_provider_test(&state, &p.id)),
        )
        .await;
        let statuses = candidates
            .iter()
            .cloned()
            .zip(results.into_iter().map(Result::ok))
            .collect::<Vec<_>>();
        let configured_status = configured.as_ref().and_then(|c| {
            statuses
                .iter()
                .find(|(p, _)| p.id == c.id)
                .and_then(|(_, s)| s.clone())
        });
        let used = statuses
            .iter()
            .find(|(_, s)| s.as_ref().is_some_and(|s| s.status == "connected"))
            .map(|(p, _)| p.clone());
        // Never send raw API keys to the browser.
        let mask = |mut provider: ProviderConfig| {
            provider.api_key = config::masked_key(&provider.api_key);
            provider
        };
        output.insert(
            name.into(),
            serde_json::to_value(ResolvedRole {
                configured_provider: configured.map(mask),
                live_status: configured_status,
                used_provider: used.map(mask),
            })
            .unwrap_or(Value::Null),
        );
    }
    Ok(Json(Value::Object(output)))
}
pub(crate) async fn put_mutis(
    State(state): State<AppState>,
    Json(value): Json<config::MutisConfig>,
) -> ApiResult<Json<config::MutisConfig>> {
    if !(1..=5).contains(&value.retry_attempts)
        || !(1..=60).contains(&value.watchdog_minutes)
        || value.resume_max > 3
        || value.fix_rounds_max > 5
        || !(50..=5000).contains(&value.deepseek_max_file_lines)
        || !(5..=40).contains(&value.deepseek_max_steps)
        || !(1..=3).contains(&value.debate_rounds)
        || value.retry_base_ms > 60000
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Giá trị ngoài giới hạn",
        ));
    }
    for rule in &value.routing {
        if !matches!(
            rule.kind.as_str(),
            "small_edit" | "code" | "test" | "docs" | "plan" | "review"
        ) || !matches!(rule.role.as_str(), "lead" | "big" | "small")
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Quy tắc định tuyến không hợp lệ",
            ));
        }
    }
    for role in [&value.roles.lead, &value.roles.big, &value.roles.small] {
        if !matches!(
            role.effort.as_str(),
            "default" | "low" | "medium" | "high" | "xhigh" | "max"
        ) {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Mức suy luận không hợp lệ",
            ));
        }
    }
    let mut config = state.config.write().await;
    config.planner_provider_id = value.roles.lead.provider_id.clone();
    config.planner_model = value.roles.lead.model.clone();
    config.mutis = value.clone();
    config.mutis.reviewer_provider_id = value.roles.lead.provider_id.clone();
    config.mutis.reviewer_model = value.roles.lead.model.clone();
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    Ok(Json(value))
}
pub(crate) async fn mutis_events(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
) -> ApiResult<Json<Vec<Event>>> {
    let limit = query
        .get("limit")
        .and_then(|x| x.parse().ok())
        .unwrap_or(200)
        .min(2000);
    state.event_log.recent(limit).map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được nhật ký MutiS",
        )
    })
}

pub(crate) async fn mutis_stats(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    let days = query
        .get("days")
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(1, 365);
    let cutoff = mutis::now().saturating_sub(days.saturating_mul(86400));
    let events = state.event_log.recent(2000).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được nhật ký MutiS",
        )
    })?;
    let events = events
        .into_iter()
        .filter(|event| event.ts >= cutoff)
        .collect::<Vec<_>>();
    let mut providers: HashMap<String, Vec<Event>> = HashMap::new();
    let mut kinds: HashMap<String, Vec<Event>> = HashMap::new();
    for event in &events {
        providers
            .entry(event.provider_id.clone())
            .or_default()
            .push(event.clone());
        kinds
            .entry(event.task_kind.clone().unwrap_or_else(|| "không rõ".into()))
            .or_default()
            .push(event.clone());
    }
    let summarize = |groups: HashMap<String, Vec<Event>>| -> Value {
        Value::Object(
            groups
                .into_iter()
                .map(|(key, group)| {
                    let starts = group.iter().filter(|e| e.kind == "task_start").count();
                    let ends = group
                        .iter()
                        .filter(|e| e.kind == "task_end")
                        .collect::<Vec<_>>();
                    let ok = ends.iter().filter(|e| e.ok).count();
                    let duration = ends.iter().filter_map(|e| e.duration_ms).sum::<u64>();
                    let tokens = ends.iter().filter_map(|e| e.tokens).sum::<u64>();
                    let mut errors: HashMap<String, usize> = HashMap::new();
                    for event in group.iter().filter(|e| !e.ok || e.kind == "error") {
                        if let Some(detail) = &event.detail {
                            *errors.entry(mutis::normalize_error(detail)).or_default() += 1;
                        }
                    }
                    let mut errors = errors.into_iter().collect::<Vec<_>>();
                    errors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                    let top_errors = errors
                        .into_iter()
                        .take(5)
                        .map(|(message, count)| json!({"message":message,"count":count}))
                        .collect::<Vec<_>>();
                    (
                        key,
                        json!({
                            "tasks_run": starts,
                            "success_rate": if ends.is_empty() {
                                0.0
                            } else {
                                ok as f64 / ends.len() as f64
                            },
                            "avg_duration_ms": if ends.is_empty() {
                                0
                            } else {
                                duration / ends.len() as u64
                            },
                            "total_tokens": tokens,
                            "retries": group.iter().filter(|e| e.kind == "retry").count(),
                            "watchdog_kills": group
                                .iter()
                                .filter(|e| e.kind == "watchdog_kill")
                                .count(),
                            "resumes": group.iter().filter(|e| e.kind == "resume").count(),
                            "verify_fail_rate": rate(&group, "verify", false),
                            "review_fail_rate": rate(&group, "review", false),
                            "fix_rounds": group.iter().filter(|e| e.kind == "fix_task").count(),
                            "top_errors": top_errors
                        }),
                    )
                })
                .collect(),
        )
    };
    Ok(Json(
        json!({"days": days, "providers": summarize(providers), "task_kinds": summarize(kinds), "events": events.into_iter().rev().take(50).collect::<Vec<_>>()}),
    ))
}
pub(crate) async fn mutis_diagnoses(State(state): State<AppState>) -> ApiResult<Json<Vec<Value>>> {
    let directory = state
        .config_path
        .parent()
        .unwrap()
        .join("mutis")
        .join("diagnoses");
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

#[derive(Deserialize)]
pub(crate) struct SettingChange {
    path: String,
    value: Value,
}
pub(crate) async fn mutis_setting_change(
    State(state): State<AppState>,
    Json(change): Json<SettingChange>,
) -> ApiResult<Json<Value>> {
    let mut config = state.config.write().await;
    let (old, new) = mutis::apply_setting_change(&mut config, &change.path, &change.value)
        .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
    if change.path == "mutis.roles.lead.provider_id" || change.path == "mutis.roles.lead.model" {
        config.planner_provider_id = config.mutis.roles.lead.provider_id.clone();
        config.planner_model = config.mutis.roles.lead.model.clone();
        config.mutis.reviewer_provider_id = config.planner_provider_id.clone();
        config.mutis.reviewer_model = config.planner_model.clone();
    }
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cấu hình"))?;
    Ok(Json(json!({"old":old,"new":new})))
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
    let mut failure_events = all_events
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
    let stats = mutis_stats(
        State(state.clone()),
        axum::extract::Query(HashMap::from([("days".to_owned(), "30".to_owned())])),
    )
    .await?
    .0;
    if insufficient {
        let diagnosis = json!({"ts":ts,"health":"chưa đủ dữ liệu","findings":[],"summary":"Chưa đủ dữ liệu để chẩn đoán"});
        save_diagnosis(&state, &diagnosis)?;
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
        let stream = async_stream::stream! {yield Ok::<_,Infallible>(axum::response::sse::Event::default().event("diagnosis").json_data(diagnosis).unwrap());};
        return Ok(Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response());
    }
    let (provider_config, model, effort) = resolve_connected_lead(&state, &config_snapshot).await?;
    let provider = providers::for_kind(&provider_config.kind)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Loại nguồn AI không hợp lệ"))?;
    let settings = serde_json::to_value(&config_snapshot.mutis).unwrap_or(Value::Null);
    let prompt = format!(
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
        serde_json::to_string(&failure_events).unwrap_or_default()
    );
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
            error: None,
        }],
        session_id: None,
        mutis: config_snapshot.mutis.clone(),
    };
    let (sender, mut receiver) = mpsc::channel::<ProviderEvent>(32);
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
    let stream_state = state.clone();
    let stream = async_stream::stream! {
        let mut raw = String::new();
        let mut failed = false;
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => raw.push_str(event.data["text"].as_str().unwrap_or_default()),
                "log" => yield Ok::<_,Infallible>(axum::response::sse::Event::default().event("log").json_data(event.data).unwrap()),
                "error" => { failed = true; yield Ok(axum::response::sse::Event::default().event("error").json_data(event.data).unwrap()); },
                _ => {}
            }
        }
        if !failed {
            match crate::core::text::extract_json(&raw).and_then(|text| serde_json::from_str::<Value>(text).ok()).and_then(|value| mutis::validate_diagnosis(value).ok()) {
                Some(value) => {
                    let diagnosis = json!({"ts":ts,"health":value["health"],"findings":value["findings"],"summary":value["summary"]});
                    if let Err(error) = save_diagnosis(&stream_state, &diagnosis) {
                        yield Ok(axum::response::sse::Event::default()
                            .event("error")
                            .json_data(json!({"message": error.message}))
                            .unwrap());
                    }
                    else {
                        let _ = stream_state.event_log.append(&Event {
                            ts,
                            plan_id: None,
                            task_id: None,
                            provider_id: provider_config.id.clone(),
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
                        yield Ok(axum::response::sse::Event::default().event("diagnosis").json_data(diagnosis).unwrap());
                    }
                }
                None => yield Ok(axum::response::sse::Event::default().event("error").json_data(json!({"message":"Không phân tích được kết quả chẩn đoán JSON"})).unwrap()),
            }
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

fn save_diagnosis(state: &AppState, diagnosis: &Value) -> ApiResult<()> {
    let dir = state
        .config_path
        .parent()
        .unwrap()
        .join("mutis")
        .join("diagnoses");
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

fn rate(events: &[Event], kind: &str, ok: bool) -> f64 {
    let matching = events.iter().filter(|e| e.kind == kind).collect::<Vec<_>>();
    if matching.is_empty() {
        0.0
    } else {
        matching.iter().filter(|e| e.ok == ok).count() as f64 / matching.len() as f64
    }
}
