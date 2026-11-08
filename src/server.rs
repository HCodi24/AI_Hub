use crate::{
    chat::{self, ChatMessage, ChatRecord, ChatRequest, ChatStore},
    config::{self, Config, ProviderConfig},
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
use futures_util::StreamExt;
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
    running: Arc<Mutex<HashMap<String, RunningChat>>>,
}

#[derive(Clone)]
struct RunningChat {
    request_id: Uuid,
    cancel: CancellationToken,
}

impl AppState {
    pub fn new(config: Config, config_path: PathBuf, chat_dir: PathBuf) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            chats: ChatStore::new(chat_dir),
            running: Arc::new(Mutex::new(HashMap::new())),
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
        .route("/api/providers", post(create_provider))
        .route(
            "/api/providers/{id}",
            put(update_provider).delete(remove_provider),
        )
        .route("/api/providers/{id}/models", get(provider_models))
        .route("/api/chat", post(chat_handler))
        .route("/api/chat/{chat_id}/stop", post(stop_chat))
        .route("/api/chats", get(list_chats))
        .route("/api/chats/{chat_id}", get(get_chat).delete(delete_chat))
        .route("/api/cli-status", get(cli_status))
        .layer(middleware::from_fn_with_state(port, enforce_local_host))
        .with_state(state)
}

async fn enforce_local_host(State(port): State<u16>, request: Request, next: Next) -> Response {
    let ipv4 = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok());
    if !host.is_some_and(|host| {
        host == ipv4 || host.eq_ignore_ascii_case(&localhost)
    }) {
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

async fn put_config(
    State(state): State<AppState>,
    Json(input): Json<ConfigInput>,
) -> ApiResult<Json<Config>> {
    let mut config = state.config.write().await;
    config.default_folder = input.default_folder.trim().to_owned();
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
    if provider.kind != "openai_compat" {
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
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Không thể giải quyết thư mục làm việc"))?
        .to_string_lossy()
        .into_owned();
    if request.model.trim().is_empty() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Hãy chọn model"));
    }
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
    record.messages = request.messages.clone();
    record.messages.push(ChatMessage {
        role: "assistant".to_owned(),
        content: String::new(),
        reasoning: String::new(),
        logs: Vec::new(),
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
        messages: request.messages,
        session_id: record.session_id.clone(),
    };
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
                    persist_event(&task_state.chats, &mut record, &event, &mut last_persist);
                    if sse_sender.send(chat::sse_event(event.name, &event.data)).await.is_err() {
                        task_cancel.cancel();
                        break;
                    }
                }
            }
        }
        let _ = chat::save_record(&task_state.chats, &mut record);
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
        "openai_compat" | "anthropic" | "codex_cli" | "claude_cli"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nhà cung cấp không hợp lệ",
        ));
    }
    if input.kind == "openai_compat" && input.base_url.trim().is_empty() {
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
