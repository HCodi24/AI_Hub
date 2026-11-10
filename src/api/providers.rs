use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{self, Config, ProviderConfig},
    providers::{self, discovery},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::timeout;
use uuid::Uuid;
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct ProviderInput {
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

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    models: Vec<String>,
}

#[derive(Serialize)]
pub(crate) struct CliStatus {
    codex_cli: CliInfo,
    claude_cli: CliInfo,
}

#[derive(Serialize)]
struct CliInfo {
    installed: bool,
    version: Option<String>,
    path: Option<String>,
}

pub(crate) async fn create_provider(
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

pub(crate) async fn update_provider(
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

pub(crate) async fn remove_provider(
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

pub(crate) async fn use_deploy_key(
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

pub(crate) async fn provider_models(
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
pub(crate) struct ConnectionTest {
    ok: bool,
    pub(crate) status: &'static str,
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
            match discovery::cli_probe(name, &provider.executable, Duration::from_secs(10)).await {
                Some((version, path)) if name == "claude" => {
                    match discovery::run_cli_auth_status(
                        std::path::Path::new(&path),
                        Duration::from_secs(10),
                    )
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

pub(crate) async fn cached_provider_test(state: &AppState, id: &str) -> ApiResult<ConnectionTest> {
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

pub(crate) async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    state.provider_status.lock().await.remove(&id);
    cached_provider_test(&state, &id).await.map(Json)
}

pub(crate) async fn login_provider(
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
pub(crate) async fn discover_cli(
    Path(name): Path<String>,
) -> ApiResult<Json<Vec<discovery::CliCandidate>>> {
    if !["codex", "claude"].contains(&name.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "CLI kh\u{00f4}ng h\u{1ee3}p l\u{1ec7}",
        ));
    }
    Ok(Json(discovery::discover_candidates(&name).await))
}

pub(crate) async fn cli_status(State(state): State<AppState>) -> Json<CliStatus> {
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
    let (installed, version, path) = discovery::probe_cli_info(name, &explicit_path).await;
    CliInfo {
        installed,
        version,
        path,
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
