use crate::{
    chat::{self, ChatMessage, ChatRecord, ChatRequest, ChatStore},
    config::{self, Config, ProviderConfig},
    mutis::{self, Event, EventLog},
    plans::{Plan, PlanStore, PlanTask},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    routing::{get, post, put},
    Json, Router,
};
use futures_util::{future::join_all, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    convert::Infallible,
    ffi::OsString,
    path::PathBuf,
    process::Stdio,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::AsyncBufReadExt,
    sync::{mpsc, Mutex, RwLock},
    time::timeout,
};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    chats: ChatStore,
    plans: PlanStore,
    running: Arc<Mutex<HashMap<String, RunningChat>>>,
    extension_running: Arc<Mutex<HashMap<String, CancellationToken>>>,
    provider_status: Arc<Mutex<HashMap<String, (Instant, ConnectionTest)>>>,
    event_log: EventLog,
}

#[derive(Clone)]
struct RunningChat {
    request_id: Uuid,
    cancel: CancellationToken,
}

impl AppState {
    pub fn new(config: Config, config_path: PathBuf, chat_dir: PathBuf) -> Self {
        let plans = PlanStore::new(&config_path).expect("config path has a parent");
        let event_log = EventLog::new(&config_path).expect("config path has a parent");
        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            chats: ChatStore::new(chat_dir),
            plans,
            running: Arc::new(Mutex::new(HashMap::new())),
            extension_running: Arc::new(Mutex::new(HashMap::new())),
            provider_status: Arc::new(Mutex::new(HashMap::new())),
            event_log,
        }
    }
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}

type ApiResult<T> = Result<T, ApiError>;

#[derive(Clone, Debug, Deserialize)]
struct ProviderInput {
    #[serde(default)]
    id: Option<String>,
    name: String,
    kind: String,
    #[serde(default)]
    base_url: String,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    executable: String,
    #[serde(default)]
    models: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ConfigInput {
    default_folder: String,
    #[serde(default)]
    planner_provider_id: String,
    #[serde(default)]
    planner_model: String,
}

#[derive(Debug, Deserialize)]
struct PlanRequest {
    request: String,
    folder: String,
    #[serde(default)]
    provider_id: String,
    #[serde(default)]
    model: String,
}

#[derive(Serialize)]
struct ModelsResponse {
    models: Vec<String>,
}

#[derive(Serialize)]
struct StopResponse {
    stopped: bool,
}

#[derive(Serialize)]
struct CliStatus {
    codex_cli: CliInfo,
    claude_cli: CliInfo,
}

#[derive(Serialize)]
struct CliInfo {
    installed: bool,
    version: Option<String>,
    path: Option<String>,
}

pub fn router(state: AppState, port: u16) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/app.js", get(app_js))
        .route("/style.css", get(style_css))
        .route("/api/config", get(get_config).put(put_config))
        .route("/api/mutis", get(get_orchestration).put(put_orchestration))
        .route("/api/mutis/events", get(orchestration_events))
        .route(
            "/api/orchestration",
            get(get_orchestration).put(put_orchestration),
        )
        .route("/api/orchestration/events", get(orchestration_events))
        .route("/api/providers", post(create_provider))
        .route(
            "/api/providers/{id}",
            put(update_provider).delete(remove_provider),
        )
        .route("/api/providers/{id}/models", get(provider_models))
        .route("/api/providers/{id}/test", post(test_provider))
        .route("/api/providers/{id}/use-deploy-key", post(use_deploy_key))
        .route("/api/providers/{id}/login", post(login_provider))
        .route("/api/cli-discover/{name}", get(discover_cli))
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/{chat_id}/stop", post(stop_chat))
        .route("/api/chats", get(list_chats))
        .route("/api/chats/{chat_id}", get(get_chat).delete(delete_chat))
        .route("/api/plans", get(list_plans).post(generate_plan))
        .route("/api/plans/{id}", get(get_plan).put(save_plan))
        .route("/api/cli-status", get(cli_status))
        .route("/api/extensions", get(extension_list))
        .route(
            "/api/extensions/{id}/actions/{action}",
            post(extension_action),
        )
        .route("/api/extensions/{id}/stop", post(extension_stop))
        .layer(middleware::from_fn_with_state(port, enforce_local_host))
        .with_state(state)
}

async fn get_orchestration(State(state): State<AppState>) -> Json<config::MutisConfig> {
    Json(state.config.read().await.mutis.clone())
}
async fn put_orchestration(
    State(state): State<AppState>,
    Json(value): Json<config::MutisConfig>,
) -> ApiResult<Json<config::MutisConfig>> {
    if !(1..=5).contains(&value.retry_attempts)
        || !(1..=60).contains(&value.watchdog_minutes)
        || value.resume_max > 3
        || value.fix_rounds_max > 5
        || !(50..=5000).contains(&value.deepseek_max_file_lines)
        || !(5..=40).contains(&value.deepseek_max_steps)
        || value.retry_base_ms > 60000
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tham số điều phối nằm ngoài phạm vi cho phép",
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
async fn orchestration_events(
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
            "Không đọc được nhật ký điều phối",
        )
    })
}

async fn enforce_local_host(State(port): State<u16>, request: Request, next: Next) -> Response {
    let ipv4 = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(|host| host == ipv4 || host.eq_ignore_ascii_case(&localhost)) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    if request.method() != axum::http::Method::GET {
        if let Some(origin) = request.headers().get("origin") {
            let origin = origin.to_str().unwrap_or_default();
            let ipv4_origin = format!("http://{ipv4}");
            let localhost_origin = format!("http://localhost:{port}");
            if !origin.eq_ignore_ascii_case(&ipv4_origin)
                && !origin.eq_ignore_ascii_case(&localhost_origin)
            {
                return (StatusCode::FORBIDDEN, "Forbidden").into_response();
            }
        }
    }
    next.run(request).await
}

async fn index() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/html; charset=utf-8")],
        include_str!("../ui/index.html"),
    )
}

async fn app_js() -> impl IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/javascript; charset=utf-8",
        )],
        include_str!("../ui/app.js"),
    )
}

async fn style_css() -> impl IntoResponse {
    (
        [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
        include_str!("../ui/style.css"),
    )
}

async fn get_config(State(state): State<AppState>) -> Json<Config> {
    let mut config = state.config.read().await.clone();
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    Json(config)
}

async fn list_plans(State(state): State<AppState>) -> ApiResult<Json<Vec<Plan>>> {
    state
        .plans
        .list()
        .map(Json)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được kế hoạch"))
}
async fn get_plan(State(state): State<AppState>, Path(id): Path<String>) -> ApiResult<Json<Plan>> {
    state
        .plans
        .get(&id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được kế hoạch"))?
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy kế hoạch"))
}
async fn save_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut plan): Json<Plan>,
) -> ApiResult<Json<Plan>> {
    if plan.id != id || !chat::valid_id(&id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã kế hoạch không hợp lệ",
        ));
    }
    if plan.tasks.iter().any(|t| {
        t.title.trim().is_empty()
            || t.detail.trim().is_empty()
            || !matches!(
                t.kind.as_str(),
                "plan" | "small_edit" | "code" | "test" | "docs" | "review"
            )
    }) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Công việc chưa hợp lệ",
        ));
    }
    if plan.approved && crate::plans::has_dependency_cycle(&plan.tasks) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Phụ thuộc công việc có chu trình",
        ));
    }
    plan.touch();
    state
        .plans
        .save(&plan)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được kế hoạch"))?;
    Ok(Json(plan))
}
async fn generate_plan(
    State(state): State<AppState>,
    Json(input): Json<PlanRequest>,
) -> ApiResult<Response> {
    let folder = PathBuf::from(input.folder.trim())
        .canonicalize()
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Working folder does not exist"))?;
    let cfg = state.config.read().await.clone();
    let statuses = join_all(
        cfg.providers
            .iter()
            .map(|p| cached_provider_test(&state, &p.id)),
    )
    .await;
    let connected =
        cfg.providers
            .iter()
            .zip(statuses)
            .filter_map(|(p, status)| {
                status.ok().filter(|s| s.status == "connected").map(|_| {
                    crate::plans::ProviderSummary {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        kind: p.kind.clone(),
                        models: p.models.clone(),
                    }
                })
            })
            .collect::<Vec<_>>();
    let requested = if !input.provider_id.is_empty() {
        input.provider_id.clone()
    } else if !cfg.planner_provider_id.is_empty() {
        cfg.planner_provider_id.clone()
    } else {
        ["claude_cli", "anthropic", "codex_cli"]
            .iter()
            .find_map(|kind| {
                connected
                    .iter()
                    .find(|p| p.kind == *kind)
                    .map(|p| p.id.clone())
            })
            .unwrap_or_default()
    };
    let planner = cfg
        .providers
        .iter()
        .find(|p| p.id == requested)
        .cloned()
        .filter(|p| connected.iter().any(|w| w.id == p.id))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Choose a connected planner provider in Settings",
            )
        });
    let planner = match planner {
        Ok(planner) => planner,
        Err(error) if cfg.mutis.enabled => {
            let fallback_kinds = ["claude_cli", "anthropic", "codex_cli"];
            connected
                .iter()
                .find(|provider| fallback_kinds.iter().any(|kind| provider.kind == *kind))
                .and_then(|provider| cfg.providers.iter().find(|item| item.id == provider.id))
                .cloned()
                .ok_or(error)?
        }
        Err(error) => return Err(error),
    };
    let model = if planner.id != requested {
        planner.models.first().cloned().unwrap_or_default()
    } else if input.model.trim().is_empty() {
        if cfg.planner_model.is_empty() {
            planner.models.first().cloned().unwrap_or_default()
        } else {
            cfg.planner_model.clone()
        }
    } else {
        input.model.clone()
    };
    if model.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Planner needs a model",
        ));
    }
    let workers = connected
        .iter()
        .map(|p| {
            format!(
                "id: {}, name: {}, kind: {}, models: {}",
                p.id,
                p.name,
                p.kind,
                p.models.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let routes = if cfg.mutis.enabled {
        format!("\nMutiS routing table (kind -> role):\n{}\nRoles: lead={}, big={}, small={}. Use deepseek_agent only for small tasks; never run commands.", cfg.mutis.routing.iter().map(|r|format!("{} -> {}",r.kind,r.role)).collect::<Vec<_>>().join("\n"), cfg.mutis.roles.lead.provider_id, cfg.mutis.roles.big.provider_id, cfg.mutis.roles.small.provider_id)
    } else {
        String::new()
    };
    let prompt = format!("Respond ONLY as JSON: {{\"title\":\"...\",\"tasks\":[{{\"id\":1,\"title\":\"...\",\"detail\":\"files + expected result\",\"assignee\":\"provider id\",\"depends_on\":[ids],\"kind\":\"plan|small_edit|code|test|docs|review\"}}]}}. Keep tasks small and avoid editing same file in parallel.\nRequest:\n{}\nWorking folder: {}\nFiles:\n{}\nConnected workers (id, name, kind, models):\n{}{}",input.request,folder.display(),file_tree(&folder),workers,routes);
    let provider = providers::for_kind(&planner.kind)
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    let cancel = CancellationToken::new();
    let (out_tx, out_rx) = mpsc::channel(32);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let request = ProviderRequest {
        provider: planner,
        model,
        folder: folder.to_string_lossy().into_owned(),
        permission: chat::Permission::Read,
        effort: chat::Effort::Default,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt,
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            error: None,
        }],
        session_id: None,
        mutis: cfg.mutis.clone(),
    };
    let task_cancel = cancel.clone();
    let task_state = state.clone();
    let planner_id = request.provider.id.clone();
    let request_text = input.request.clone();
    let folder_text = folder.to_string_lossy().into_owned();
    tokio::spawn(async move {
        let run_cancel = task_cancel.clone();
        let tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = provider.run(request, tx.clone(), run_cancel).await {
                let _ = tx.send(ProviderEvent::text("error", e.to_string())).await;
            }
        });
        // Drop our sender so `event_rx.recv()` returns None once the provider task finishes.
        drop(event_tx);
        let mut raw = String::new();
        let mut error = None;
        loop {
            tokio::select! {
                _ = task_cancel.cancelled() => return,
                _ = out_tx.closed() => { task_cancel.cancel(); return; },
                event = event_rx.recv() => {
                    let Some(event) = event else { break };
                    if event.name == "delta" {
                        raw.push_str(event.data["text"].as_str().unwrap_or_default());
                    } else if event.name == "error" {
                        error = Some(event.data["text"].as_str().unwrap_or("planner failed").to_owned());
                    } else if event.name == "log"
                        && out_tx.send(chat::sse_event("log", &event.data)).await.is_err()
                    {
                        task_cancel.cancel();
                        return;
                    }
                }
            }
        }
        let result =
            (|| -> Result<Plan, String> {
                if let Some(e) = error {
                    return Err(e);
                };
                let parsed = extract_json(&raw)
                    .and_then(|v| serde_json::from_str::<Value>(v).ok())
                    .ok_or_else(|| format!("Could not parse planner JSON: {raw}"))?;
                let source = parsed["tasks"]
                    .as_array()
                    .ok_or_else(|| "Missing tasks array".to_owned())?;
                let tasks =
                    source
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let old = t["assignee"].as_str().unwrap_or(&planner_id).to_owned();
                            let kind = t["kind"].as_str().unwrap_or("code").to_owned();
                            let valid = connected.iter().any(|p| p.id == old);
                            let assignee = if cfg.mutis.enabled {
                                crate::plans::reassign_with_routing(
                                    &kind,
                                    &connected,
                                    &planner_id,
                                    &cfg.mutis.routing,
                                    &cfg.mutis.roles,
                                    cfg.mutis.enabled,
                                )
                            } else if valid {
                                old.clone()
                            } else {
                                crate::plans::reassign_with_routing(
                                    &kind,
                                    &connected,
                                    &planner_id,
                                    &cfg.mutis.routing,
                                    &cfg.mutis.roles,
                                    false,
                                )
                            };
                            let role = cfg.mutis.routing.iter().find(|rule| rule.kind == kind).map(
                                |rule| match rule.role.as_str() {
                                    "big" => &cfg.mutis.roles.big,
                                    "small" => &cfg.mutis.roles.small,
                                    _ => &cfg.mutis.roles.lead,
                                },
                            );
                            let assigned_provider =
                                connected.iter().find(|provider| provider.id == assignee);
                            let routed_role = role.filter(|role| role.provider_id == assignee);
                            let reassigned_from = if assignee == old { None } else { Some(old) };
                            PlanTask {
                                id: t["id"].as_u64().unwrap_or(i as u64 + 1) as u32,
                                title: t["title"].as_str().unwrap_or("Task").into(),
                                detail: t["detail"].as_str().unwrap_or("").into(),
                                assignee,
                                model: routed_role
                                    .map(|role| role.model.clone())
                                    .or_else(|| t["model"].as_str().map(str::to_owned))
                                    .filter(|model| !model.is_empty())
                                    .or_else(|| {
                                        assigned_provider
                                            .and_then(|provider| provider.models.first().cloned())
                                    }),
                                effort: routed_role.map(|role| role.effort.clone()).unwrap_or_else(
                                    || t["effort"].as_str().unwrap_or("default").into(),
                                ),
                                reassigned_from,
                                depends_on: t["depends_on"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_u64)
                                    .map(|v| v as u32)
                                    .collect(),
                                kind,
                                status: "chờ".into(),
                                summary: String::new(),
                                log: Vec::new(),
                                tokens: 0,
                                chat_id: None,
                            }
                        })
                        .collect();
                Ok(Plan::new(
                    parsed["title"].as_str().unwrap_or("New plan").into(),
                    folder_text,
                    request_text,
                    tasks,
                ))
            })();
        match result {
            Ok(plan) => match task_state.plans.save(&plan) {
                Ok(()) => {
                    let _ = out_tx.send(chat::sse_event("plan", &json!(plan))).await;
                }
                Err(e) => {
                    let _ = out_tx
                        .send(chat::sse_event("error", &json!({"message":e.to_string()})))
                        .await;
                }
            },
            Err(e) => {
                let _ = out_tx
                    .send(chat::sse_event("error", &json!({"message":e})))
                    .await;
            }
        }
    });
    let stream_cancel = cancel.clone();
    let stream = async_stream::stream! {let _guard=CancelOnDrop(stream_cancel);let mut rx=ReceiverStream::new(out_rx);while let Some(event)=rx.next().await{yield Ok::<_,Infallible>(event)}};
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
fn extract_json(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let mut depth = 0;
    let mut quoted = false;
    let mut escape = false;
    for (offset, c) in raw[start..].char_indices() {
        if quoted {
            if escape {
                escape = false
            } else if c == '\\' {
                escape = true
            } else if c == '"' {
                quoted = false
            }
        } else if c == '"' {
            quoted = true
        } else if c == '{' {
            depth += 1
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                return Some(&raw[start..start + offset + c.len_utf8()]);
            }
        }
    }
    None
}

#[cfg(test)]
mod plan_tests {
    use super::extract_json;
    use serde_json::Value;

    #[test]
    fn extracts_json_from_planner_answer_with_surrounding_text() {
        let answer = "Here is the plan:\n```json\n{\"title\":\"Ship it\",\"tasks\":[{\"id\":1}]}\n```\nDone.";
        let extracted = extract_json(answer).expect("JSON object should be extracted");
        let value: Value = serde_json::from_str(extracted).expect("extracted JSON should parse");
        assert_eq!(value["title"], "Ship it");
        assert_eq!(value["tasks"][0]["id"], 1);
    }
}
fn file_tree(root: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        if out.len() >= 200 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if out.len() >= 200 {
                    break;
                }
                let path = entry.path();
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
                if path.is_dir()
                    && [".git", "target", "node_modules", ".codegraph"]
                        .iter()
                        .any(|name| path.file_name().is_some_and(|n| n == *name))
                {
                    continue;
                }
                out.push(if path.is_dir() {
                    format!("{rel}/")
                } else {
                    rel.to_string()
                });
                if path.is_dir() {
                    walk(&path, root, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.join("\n")
}

async fn put_config(
    State(state): State<AppState>,
    Json(input): Json<ConfigInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    config.default_folder = input.default_folder.trim().to_owned();
    config.planner_provider_id = input.planner_provider_id;
    config.planner_model = input.planner_model;
    config.mutis.roles.lead.provider_id = config.planner_provider_id.clone();
    config.mutis.roles.lead.model = config.planner_model.clone();
    config.mutis.reviewer_provider_id = config.planner_provider_id.clone();
    config.mutis.reviewer_model = config.planner_model.clone();
    config::save(&state.config_path, &config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
    let mut response = config.clone();
    for provider in &mut response.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    Ok(Json(response))
}

async fn create_provider(
    State(state): State<AppState>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    validate_provider(&id, &input)?;
    if config.providers.iter().any(|provider| provider.id == id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Mã nhà cung cấp đã tồn tại",
        ));
    }
    let provider = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key: input.api_key.unwrap_or_default(),
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
    };
    config.providers.push(provider);
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

async fn update_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProviderInput>,
) -> ApiResult<Json<Config>> {
    validate_provider(&id, &input)?;
    let mut config = state.config.write().await;
    let Some(index) = config
        .providers
        .iter()
        .position(|provider| provider.id == id)
    else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    };
    let old_key = config.providers[index].api_key.clone();
    let submitted_key = input.api_key.unwrap_or_default();
    let api_key = if submitted_key.trim().is_empty() || config::is_masked_key(&submitted_key) {
        old_key
    } else {
        submitted_key
    };
    config.providers[index] = ProviderConfig {
        id,
        name: input.name.trim().to_owned(),
        kind: input.kind,
        base_url: input.base_url.trim().trim_end_matches('/').to_owned(),
        api_key,
        executable: input.executable.trim().to_owned(),
        models: clean_models(input.models),
    };
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

async fn remove_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let previous_len = config.providers.len();
    config.providers.retain(|provider| provider.id != id);
    if config.providers.len() == previous_len {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy nhà cung cấp",
        ));
    }
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

async fn use_deploy_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    let key = config
        .providers
        .iter()
        .find(|p| p.id == "deploy-deepseek" && p.kind == "openai_compat")
        .map(|p| p.api_key.clone())
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Chưa có key trong nguồn Deploy (DeepSeek)",
            )
        })?;
    let target = config
        .providers
        .iter_mut()
        .find(|p| p.id == id && p.kind == "deepseek_agent")
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::NOT_FOUND,
                "Không tìm thấy nguồn DeepSeek làm việc",
            )
        })?;
    target.api_key = key;
    persist_config(&state, &config)?;
    Ok(Json(masked_config(&config)))
}

async fn provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ModelsResponse>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    if !matches!(provider.kind.as_str(), "openai_compat" | "deepseek_agent") {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho OpenAI-compatible",
        ));
    }
    if provider.api_key.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy lưu API key trước khi tải model",
        ));
    }
    let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
    let response = reqwest::Client::new()
        .get(url)
        .bearer_auth(&provider.api_key)
        .send()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Không thể kết nối tới API models"))?;
    if !response.status().is_success() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            format!("API models trả về HTTP {}", response.status()),
        ));
    }
    let body: Value = response
        .json()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_GATEWAY, "Phản hồi models không hợp lệ"))?;
    let mut models = body["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str().map(str::to_owned))
        .collect::<Vec<_>>();
    models.sort();
    models.dedup();
    {
        let mut config = state.config.write().await;
        if let Some(entry) = config.providers.iter_mut().find(|entry| entry.id == id) {
            entry.models = models.clone();
            persist_config(&state, &config)?;
        }
    }
    Ok(Json(ModelsResponse { models }))
}

#[derive(Clone, Serialize)]
struct ConnectionTest {
    ok: bool,
    status: &'static str,
    message: String,
    detail: String,
}

async fn test_provider_uncached(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    let result = match provider.kind.as_str() {
        "openai_compat" | "deepseek_agent" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "openai_compat" | "deepseek_agent" => {
            let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
            match timeout(
                Duration::from_secs(15),
                reqwest::Client::new()
                    .get(url)
                    .bearer_auth(&provider.api_key)
                    .send(),
            )
            .await
            {
                Err(_) => connection(
                    false,
                    "error",
                    "Không kết nối được",
                    "Hết thời gian chờ kết nối",
                ),
                Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
                Ok(Ok(response)) if response.status().is_success() => {
                    let detail = timeout(Duration::from_secs(15), response.json::<Value>())
                        .await
                        .ok()
                        .and_then(Result::ok)
                        .unwrap_or(Value::Null);
                    let count = detail["data"].as_array().map_or(0, Vec::len);
                    connection(
                        true,
                        "connected",
                        &format!("Đã kết nối · {count} model"),
                        &format!("Đã nhận {count} model từ {}", provider.base_url),
                    )
                }
                Ok(Ok(response)) => match response.status().as_u16() {
                    401 | 403 => connection(
                        false,
                        "error",
                        "Sai API key",
                        &format!("HTTP {}", response.status()),
                    ),
                    404 => connection(false, "error", "Sai Base URL", "HTTP 404"),
                    _ => connection(
                        false,
                        "error",
                        "Không kết nối được",
                        &format!("HTTP {}", response.status()),
                    ),
                },
            }
        }
        "anthropic" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "anthropic" => match timeout(
            Duration::from_secs(15),
            reqwest::Client::new()
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01")
                .send(),
        )
        .await
        {
            Err(_) => connection(
                false,
                "error",
                "Không kết nối được",
                "Hết thời gian chờ kết nối",
            ),
            Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
            Ok(Ok(r)) if r.status().is_success() => connection(
                true,
                "connected",
                "Đã kết nối",
                "Anthropic API phản hồi thành công",
            ),
            Ok(Ok(r)) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => connection(
                false,
                "error",
                "Sai API key",
                &format!("HTTP {}", r.status()),
            ),
            Ok(Ok(r)) => connection(
                false,
                "error",
                "Không kết nối được",
                &format!("HTTP {}", r.status()),
            ),
        },
        "codex_cli" | "claude_cli" => {
            let name = if provider.kind == "codex_cli" {
                "codex"
            } else {
                "claude"
            };
            match cli_probe(name, &provider.executable, Duration::from_secs(10)).await {
                Some((version, path)) if name == "claude" => {
                    match run_cli_auth_status(std::path::Path::new(&path), Duration::from_secs(10))
                        .await
                    {
                        Some(true) => {
                            connection(true, "connected", &format!("Đã kết nối · {version}"), &path)
                        }
                        Some(false) => {
                            connection(false, "error", "Claude Code chưa đăng nhập", &path)
                        }
                        None => {
                            connection(true, "connected", &format!("Đã kết nối · {version}"), &path)
                        }
                    }
                }
                Some((version, path)) => connection(
                    true,
                    "connected",
                    &format!("Đã kết nối · {version} · {path}"),
                    &path,
                ),
                None if providers::cli_support::resolve_executable(
                    name,
                    Some(&provider.executable),
                )
                .is_none() =>
                {
                    connection(
                        false,
                        "not_configured",
                        if name == "codex" {
                            "Chưa cài Codex CLI"
                        } else {
                            "Chưa cài Claude Code CLI"
                        },
                        if name == "codex" {
                            "npm i -g @openai/codex"
                        } else {
                            "npm i -g @anthropic-ai/claude-code"
                        },
                    )
                }
                None => connection(
                    false,
                    "error",
                    "Không kết nối được",
                    "Không chạy được --version trong 10 giây hoặc phản hồi không hợp lệ",
                ),
            }
        }
        _ => connection(
            false,
            "error",
            "Không kết nối được",
            "Loại nhà cung cấp không được hỗ trợ",
        ),
    };
    Ok(Json(result))
}

async fn cached_provider_test(state: &AppState, id: &str) -> ApiResult<ConnectionTest> {
    if let Some((checked, status)) = state.provider_status.lock().await.get(id).cloned() {
        if checked.elapsed() < Duration::from_secs(60) {
            return Ok(status);
        }
    }
    let status = test_provider_uncached(State(state.clone()), Path(id.to_owned()))
        .await?
        .0;
    state
        .provider_status
        .lock()
        .await
        .insert(id.to_owned(), (Instant::now(), status.clone()));
    Ok(status)
}

async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    state.provider_status.lock().await.remove(&id);
    cached_provider_test(&state, &id).await.map(Json)
}

async fn login_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Provider not found"))?;
    if provider.kind != "claude_cli" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Login is only available for Claude Code CLI",
        ));
    }
    let path = providers::cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Claude Code CLI not found"))?;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new("cmd");
        command.args(["/D", "/S", "/C"]);
        let raw = String::from("start \"Claude login\" \"%AI_HUB_CLI_EXECUTABLE%\" auth login");
        command.env("AI_HUB_CLI_EXECUTABLE", &path);
        for (name, _) in std::env::vars() {
            let upper = name.to_ascii_uppercase();
            if upper.starts_with("CLAUDE") || upper.starts_with("ANTHROPIC_") {
                command.env_remove(name);
            }
        }
        command.raw_arg(format!("\"{raw}\""));
        command
            .spawn()
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    }
    #[cfg(not(windows))]
    {
        let mut command = providers::cli_support::command_for(
            &path,
            &[OsString::from("auth"), OsString::from("login")],
        );
        providers::cli_support::clean_claude_environment(&mut command);
        command
            .spawn()
            .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    }
    state.provider_status.lock().await.remove(&id);
    Ok(StatusCode::ACCEPTED)
}
fn connection(ok: bool, status: &'static str, message: &str, detail: &str) -> ConnectionTest {
    ConnectionTest {
        ok,
        status,
        message: message.into(),
        detail: detail.into(),
    }
}
async fn cli_probe(name: &str, explicit: &str, limit: Duration) -> Option<(String, String)> {
    let path = providers::cli_support::resolve_executable(name, Some(explicit))?;
    if unsafe_cli_path(&path) {
        return None;
    }
    let text = run_cli_version(&path, limit).await?;
    let valid = if name == "codex" {
        text.to_lowercase().contains("codex")
    } else {
        text.contains("Claude Code")
    };
    valid.then(|| (text, path.to_string_lossy().into_owned()))
}
fn unsafe_cli_path(path: &std::path::Path) -> bool {
    let text = path.to_string_lossy().to_lowercase();
    if text.contains("\\program files\\windowsapps\\") {
        return true;
    }
    if path
        .file_name()
        .is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case("Claude.exe"))
    {
        return path
            .parent()
            .is_some_and(|p| p.join("resources").join("app.asar").exists());
    }
    false
}
async fn run_cli_version(path: &std::path::Path, limit: Duration) -> Option<String> {
    let mut command = providers::cli_support::command_for(path, &[OsString::from("--version")]);
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("claude.exe"))
        || path.to_string_lossy().to_lowercase().contains("claude")
    {
        providers::cli_support::clean_claude_environment(&mut command);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(limit, command.output()).await.ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    (!value.is_empty()).then(|| value.chars().take(240).collect())
}

async fn run_cli_auth_status(path: &std::path::Path, limit: Duration) -> Option<bool> {
    let mut command = providers::cli_support::command_for(
        path,
        &[OsString::from("auth"), OsString::from("status")],
    );
    providers::cli_support::clean_claude_environment(&mut command);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(limit, command.output()).await.ok()?.ok()?;
    // `claude auth status` exits with code 1 when logged out but still prints the JSON,
    // so parse stdout regardless of the exit status.
    providers::cli_support::parse_auth_logged_in(&output.stdout)
}

#[derive(Serialize)]
struct CliCandidate {
    path: String,
    version: String,
    source: String,
}
async fn discover_cli(Path(name): Path<String>) -> ApiResult<Json<Vec<CliCandidate>>> {
    if !["codex", "claude"].contains(&name.as_str()) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "CLI không hợp lệ"));
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let search_name = name.clone();
    let found = timeout(
        deadline.saturating_duration_since(Instant::now()),
        tokio::task::spawn_blocking(move || discovery_paths(&search_name)),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();
    let mut results = Vec::new();
    for (path, source) in found.into_iter().take(200) {
        if Instant::now() >= deadline {
            break;
        }
        if unsafe_cli_path(&path) {
            continue;
        }
        if let Some(version) = run_cli_version(
            &path,
            (deadline - Instant::now()).min(Duration::from_secs(10)),
        )
        .await
        {
            let matches = if name == "codex" {
                version.to_lowercase().contains("codex")
            } else {
                version.contains("Claude Code")
            };
            if matches
                && !results.iter().any(|c: &CliCandidate| {
                    c.version == version
                        && canonical_install(PathBuf::from(&c.path))
                            == canonical_install(path.clone())
                })
            {
                let path_text = path.to_string_lossy().replace('/', "\\");
                results.push(CliCandidate {
                    path: path_text,
                    version,
                    source,
                });
            }
        }
    }
    Ok(Json(drop_bundled_cli_duplicates(results)))
}
fn drop_bundled_cli_duplicates(candidates: Vec<CliCandidate>) -> Vec<CliCandidate> {
    let command_candidates = candidates
        .iter()
        .filter(|candidate| {
            PathBuf::from(&candidate.path)
                .extension()
                .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("cmd"))
        })
        .map(|candidate| {
            (
                candidate.version.clone(),
                PathBuf::from(&candidate.path)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .join("node_modules"),
            )
        })
        .collect::<Vec<_>>();

    candidates
        .into_iter()
        .filter(|candidate| {
            let path = PathBuf::from(&candidate.path);
            !command_candidates.iter().any(|(version, node_modules)| {
                if candidate.version != *version {
                    return false;
                }
                let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
                let node_modules = node_modules
                    .to_string_lossy()
                    .replace('/', "\\")
                    .trim_end_matches('\\')
                    .to_lowercase();
                path.starts_with(&format!("{node_modules}\\"))
            })
        })
        .collect()
}
fn canonical_install(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}
fn discovery_paths(name: &str) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut add = |p: PathBuf, s: &str| {
        if p.is_file() && !out.iter().any(|(x, _)| x == &p) {
            out.push((p, s.into()));
        }
    };
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for n in [format!("{name}.cmd"), format!("{name}.exe"), name.into()] {
            add(dir.join(n), "PATH");
        }
    }
    if let Some(path) = providers::cli_support::resolve_executable(name, None) {
        add(path, "resolved executable");
    }
    #[cfg(windows)]
    {
        let app = std::env::var_os("APPDATA").map(PathBuf::from);
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let user = std::env::var_os("USERPROFILE").map(PathBuf::from);
        if let Some(a) = app.as_ref() {
            add(a.join("npm").join(format!("{name}.cmd")), "npm");
            if name == "codex" {
                let vendor = a
                    .join("npm")
                    .join("node_modules")
                    .join("@openai")
                    .join("codex")
                    .join("node_modules")
                    .join("@openai")
                    .join("codex-win32-x64")
                    .join("vendor");
                if let Ok(entries) = std::fs::read_dir(vendor) {
                    for e in entries.flatten().take(200) {
                        add(e.path().join("bin/codex.exe"), "npm bundled");
                    }
                }
            }
        }
        if let Some(u) = user {
            if name == "claude" {
                add(
                    u.join(".local").join("bin").join("claude.exe"),
                    "user local bin",
                );
                add(
                    u.join(".claude").join("local").join("claude"),
                    "Claude Code",
                );
                add(
                    u.join(".claude").join("local").join("claude.exe"),
                    "Claude Code",
                );
                add(
                    u.join(".claude").join("local").join("claude.cmd"),
                    "Claude Code",
                );
            }
        }
        if let Some(l) = local.as_ref() {
            if name == "claude" {
                let root = l.join("Claude-Data");
                let mut app_candidates = Vec::new();
                if let Ok(installs) = std::fs::read_dir(root) {
                    for install in installs.flatten().take(200) {
                        let versions = install.path().join("claude-code");
                        if let Ok(entries) = std::fs::read_dir(versions) {
                            for version in entries.flatten().take(200) {
                                let Some(parsed) = providers::cli_support::version_dir_key(
                                    &version.file_name().to_string_lossy(),
                                ) else {
                                    continue;
                                };
                                if let Ok(hashes) = std::fs::read_dir(version.path()) {
                                    for hash in hashes.flatten().take(200) {
                                        let executable = hash.path().join("claude.exe");
                                        if executable.is_file() {
                                            app_candidates.push((parsed.clone(), executable));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                app_candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
                for (_, path) in app_candidates {
                    add(path, "Claude app");
                }
            }
            if name == "codex" {
                let root = l.join("OpenAI/Codex/bin");
                if let Ok(entries) = std::fs::read_dir(root) {
                    for e in entries.flatten().take(200) {
                        add(e.path().join("codex.exe"), "Codex app");
                    }
                }
            }
        }
        if let Some(l) = local {
            let root = l.join("Packages");
            if let Ok(packages) = std::fs::read_dir(root) {
                for package in packages.flatten().take(200) {
                    add(
                        package
                            .path()
                            .join(format!("LocalCache/Roaming/npm/{name}.cmd")),
                        "package npm",
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod cli_discovery_tests {
    use super::{drop_bundled_cli_duplicates, CliCandidate};

    #[test]
    fn drops_same_version_binary_inside_cmd_node_modules() {
        let candidates = vec![
            CliCandidate {
                path: r"C:\Users\test\npm\codex.cmd".into(),
                version: "codex-cli 0.167.0".into(),
                source: "PATH".into(),
            },
            CliCandidate {
                path: r"C:\Users\test\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe".into(),
                version: "codex-cli 0.167.0".into(),
                source: "npm bundled".into(),
            },
            CliCandidate {
                path: r"C:\Other\codex.exe".into(),
                version: "codex-cli 0.167.0".into(),
                source: "other".into(),
            },
            CliCandidate {
                path: r"C:\Users\test\npm\node_modules\codex-next\bin\codex.exe".into(),
                version: "codex-cli 0.168.0".into(),
                source: "npm bundled".into(),
            },
        ];

        let retained = drop_bundled_cli_duplicates(candidates);
        assert_eq!(retained.len(), 3);
        assert!(retained
            .iter()
            .all(|candidate| !candidate.path.contains("codex-win32-x64")));
    }
}

async fn chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> ApiResult<Response> {
    if !chat::valid_id(&request.chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    let folder = request.folder.trim();
    if folder.is_empty() || !PathBuf::from(folder).is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = PathBuf::from(folder)
        .canonicalize()
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

    let mut record = state
        .chats
        .get(&request.chat_id)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không thể đọc lịch sử chat",
            )
        })?
        .unwrap_or_else(|| chat::new_record(&request));
    record.provider_id = request.provider_id.clone();
    record.model = request.model.clone();
    record.folder = folder.clone();
    record.permission = request.permission;
    record.effort = effort;
    record.messages = request.messages.clone();
    record.messages.push(ChatMessage {
        role: "assistant".to_owned(),
        content: String::new(),
        reasoning: String::new(),
        logs: Vec::new(),
        diff: String::new(),
        error: None,
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
        messages: request.messages,
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
            ok: true,
            duration_ms: None,
            tokens: None,
            detail: None,
        });
    }
    let task_cancel = cancel.clone();
    tokio::spawn(async move {
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
                        let _=task_state.event_log.append(&Event{ts:mutis::now(),plan_id:plan_id.clone(),task_id:task_id.clone(),provider_id:event_provider_id.clone(),kind:"watchdog_kill".into(),ok:false,duration_ms:None,tokens:None,detail:Some("Codex watchdog".into())});
                    }
                    if event_logging_enabled && event.name == "retry" {
                        let _=task_state.event_log.append(&Event{ts:mutis::now(),plan_id:plan_id.clone(),task_id:task_id.clone(),provider_id:event_provider_id.clone(),kind:"retry".into(),ok:true,duration_ms:None,tokens:None,detail:event.data["text"].as_str().map(str::to_owned)});
                    }
                    if event_logging_enabled && event.data["text"].as_str().is_some_and(|text|text.starts_with("Tự tiếp tục lần ")) {
                        let _=task_state.event_log.append(&Event{ts:mutis::now(),plan_id:plan_id.clone(),task_id:task_id.clone(),provider_id:event_provider_id.clone(),kind:"resume".into(),ok:true,duration_ms:None,tokens:None,detail:event.data["text"].as_str().map(str::to_owned)});
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

async fn stop_chat(
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

async fn list_chats(State(state): State<AppState>) -> ApiResult<Json<Vec<chat::ChatSummary>>> {
    state.chats.list().map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể liệt kê lịch sử chat",
        )
    })
}

async fn get_chat(
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

async fn delete_chat(
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

async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
    let config = state.config.read().await;
    let codex_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
    let claude_executable = config
        .providers
        .iter()
        .find(|provider| provider.kind == "claude_cli")
        .map(|provider| provider.executable.clone())
        .unwrap_or_default();
    drop(config);
    let (codex_cli, claude_cli) = tokio::join!(
        cli_info("codex", codex_executable),
        cli_info("claude", claude_executable)
    );
    Json(CliStatus {
        codex_cli,
        claude_cli,
    })
}

async fn cli_info(name: &str, explicit_path: String) -> CliInfo {
    let explicit_path = explicit_path.trim();
    let Some(path) = providers::cli_support::resolve_executable(name, Some(explicit_path)) else {
        return CliInfo {
            installed: false,
            version: None,
            path: None,
        };
    };
    let path_text = path.to_string_lossy().into_owned();
    let mut command = providers::cli_support::command_for(&path, &[OsString::from("--version")]);
    if name == "claude" {
        providers::cli_support::clean_claude_environment(&mut command);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let version = match timeout(Duration::from_secs(5), command.output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                return CliInfo {
                    installed: true,
                    version: None,
                    path: Some(path_text),
                };
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let value = if stdout.trim().is_empty() {
                stderr.trim()
            } else {
                stdout.trim()
            };
            if value.is_empty() {
                None
            } else {
                Some(value.chars().take(240).collect())
            }
        }
        _ => None,
    };
    CliInfo {
        installed: true,
        version,
        path: Some(path_text),
    }
}

#[derive(Deserialize)]
struct ExtensionQuery {
    folder: Option<String>,
}
#[derive(Deserialize)]
struct ExtensionActionInput {
    folder: Option<String>,
}
async fn extension_list(
    axum::extract::Query(q): axum::extract::Query<ExtensionQuery>,
) -> Json<Vec<crate::extensions::Card>> {
    Json(crate::extensions::status(q.folder.as_deref()).await)
}
async fn extension_stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<StopResponse> {
    let token = state.extension_running.lock().await.get(&id).cloned();
    if let Some(token) = token {
        token.cancel();
        Json(StopResponse { stopped: true })
    } else {
        Json(StopResponse { stopped: false })
    }
}
async fn extension_action(
    State(state): State<AppState>,
    Path((id, action_id)): Path<(String, String)>,
    Json(input): Json<ExtensionActionInput>,
) -> ApiResult<Response> {
    let Some((definition, action)) = crate::extensions::action(&id, &action_id) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Extension/action không tồn tại",
        ));
    };
    let mut running = state.extension_running.lock().await;
    if running.contains_key(&id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Extension đang có tác vụ",
        ));
    }
    let token = CancellationToken::new();
    running.insert(id.clone(), token.clone());
    drop(running);
    let mut folder = None;
    if action.needs_folder {
        let Some(raw) = input.folder.as_deref() else {
            state.extension_running.lock().await.remove(&id);
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Cần thư mục hiện tại",
            ));
        };
        let path = PathBuf::from(raw);
        if !path.is_dir() || raw.contains('%') {
            state.extension_running.lock().await.remove(&id);
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Thư mục không hợp lệ hoặc không tồn tại",
            ));
        }
        folder = Some(path);
    }
    let (tx, rx) = mpsc::channel::<Result<String, String>>(64);
    let state2 = state.clone();
    let id2 = id.clone();
    let exe = crate::extensions::resolve(action.command).or_else(|| {
        if action.command == "npm" {
            crate::extensions::resolve("npm.cmd")
        } else {
            None
        }
    });
    let args = action
        .args
        .iter()
        .map(|s| OsString::from(*s))
        .collect::<Vec<_>>();
    let backup = action.backup.map(str::to_owned);
    let modify = action.modifies.to_owned();
    tokio::spawn(async move {
        let result=async {
            if let Some(file)=backup.as_deref(){if let Some(home)=dirs::home_dir(){let source=home.join(file);if source.exists(){let dir=dirs::config_dir().unwrap_or_else(||home.clone()).join("ai-hub").join("backups");tokio::fs::create_dir_all(&dir).await.map_err(|e|e.to_string())?;let stamp=backup_timestamp();let target=dir.join(format!("{}.{}.bak",source.file_name().unwrap_or_default().to_string_lossy(),stamp));tokio::fs::copy(&source,&target).await.map_err(|e|e.to_string())?;let _=tx.send(Ok(format!("Backup: {}",target.display()))).await;}}}
            let path=exe.ok_or_else(||format!("Không tìm thấy executable: {}",action.command))?;
            let _=tx.send(Ok(format!("{} {} | có thể sửa: {}",path.display(),args.iter().map(|x|x.to_string_lossy()).collect::<Vec<_>>().join(" "),modify))).await;
            let mut cmd=providers::cli_support::command_for(&path,&args);if let Some(cwd)=folder.as_ref(){cmd.current_dir(cwd);}cmd.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
            let mut child=cmd.spawn().map_err(|e|e.to_string())?;let stdout=child.stdout.take().ok_or_else(||"Không đọc được output".to_string())?;let stderr=child.stderr.take();if let Some(stderr)=stderr{let tx_err=tx.clone();tokio::spawn(async move{let mut reader=tokio::io::BufReader::new(stderr);let mut line=String::new();loop{line.clear();if reader.read_line(&mut line).await.unwrap_or(0)==0{break}let _=tx_err.send(Ok(strip_ansi(&line))).await;}});}
            let mut reader=tokio::io::BufReader::new(stdout);let mut line=String::new();
            let wait=async{let deadline=tokio::time::sleep(Duration::from_secs(600));tokio::pin!(deadline);loop{line.clear();tokio::select!{_ = token.cancelled()=>{providers::cli_support::terminate_child_tree(&mut child).await;return Err("Đã dừng".to_string())},_ = &mut deadline=>{providers::cli_support::terminate_child_tree(&mut child).await;return Err("Quá thời gian chờ 10 phút".to_string())},read=reader.read_line(&mut line)=>{let n=read.map_err(|e|e.to_string())?;if n==0{break}let _=tx.send(Ok(strip_ansi(&line))).await;}}}let status=child.wait().await.map_err(|e|e.to_string())?;if status.success(){Ok(())}else{Err(format!("Lệnh kết thúc với {status}"))}}.await;
            wait
        }.await;
        let _ = tx.send(Err(result.err().unwrap_or_default())).await;
        state2.extension_running.lock().await.remove(&id2);
    });
    let stream = ReceiverStream::new(rx).flat_map(|item| {
        let events = match item {
            Ok(line) => vec![axum::response::sse::Event::default()
                .event("log")
                .data(json!(line).to_string())],
            Err(error) => {
                let mut out = Vec::new();
                if !error.is_empty() {
                    out.push(
                        axum::response::sse::Event::default()
                            .event("error")
                            .data(json!({"message":error}).to_string()),
                    );
                }
                out.push(
                    axum::response::sse::Event::default()
                        .event("done")
                        .data(json!({"ok":error.is_empty()}).to_string()),
                );
                out
            }
        };
        tokio_stream::iter(events.into_iter().map(Ok::<_, Infallible>))
    });
    let _ = definition;
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
fn strip_ansi(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for x in chars.by_ref() {
                if ('@'..='~').contains(&x) {
                    break;
                }
            }
        } else {
            out.push(c)
        }
    }
    out
}
fn backup_timestamp() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86400);
    let rem = seconds.rem_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}
#[cfg(test)]
mod extension_tests {
    #[test]
    fn ansi_sequences_are_removed() {
        assert_eq!(super::strip_ansi("\u{1b}[31mred\u{1b}[0m"), "red");
    }
    #[test]
    fn backup_timestamp_has_required_shape() {
        let stamp = super::backup_timestamp();
        assert_eq!(stamp.len(), 15);
        assert_eq!(stamp.as_bytes()[8], b'-');
    }
}

fn validate_provider(id: &str, input: &ProviderInput) -> ApiResult<()> {
    if !valid_provider_id(id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã nhà cung cấp không hợp lệ",
        ));
    }
    if input.name.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tên nhà cung cấp không được để trống",
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "openai_compat" | "deepseek_agent" | "anthropic" | "codex_cli" | "claude_cli"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nhà cung cấp không hợp lệ",
        ));
    }
    if matches!(input.kind.as_str(), "openai_compat" | "deepseek_agent")
        && input.base_url.trim().is_empty()
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "OpenAI-compatible cần URL cơ sở",
        ));
    }
    Ok(())
}

fn valid_provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

fn clean_models(models: Vec<String>) -> Vec<String> {
    models
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect()
}

fn masked_config(config: &Config) -> Config {
    let mut config = config.clone();
    for provider in &mut config.providers {
        provider.api_key = config::masked_key(&provider.api_key);
    }
    config
}

fn persist_config(state: &AppState, config: &Config) -> ApiResult<()> {
    config::save(&state.config_path, config)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))
}
