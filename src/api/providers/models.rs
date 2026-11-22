//! Model list of a source, and the usage limits of a CLI source.
use super::crud::persist_config;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ModelEffort,
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::Value;
use std::{collections::BTreeMap, ffi::OsString, path::Path as FsPath, process::Stdio, time::Duration};

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    models: Vec<String>,
}

pub(crate) async fn provider_limits(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<providers::limits::Limits>> {
    let (provider, ask_anthropic) = {
        let config = state.config.read().await;
        let provider = config.providers.iter().find(|p| p.id == id).cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Provider not found"))?;
        (provider, config.claude_usage_api)
    };
    Ok(Json(providers::limits::read(&provider, ask_anthropic).await))
}

#[derive(Default)]
struct ModelCatalog {
    ids: Vec<String>,
    efforts: BTreeMap<String, ModelEffort>,
}

impl ModelCatalog {
    fn add_codex(&mut self, row: &Value) {
        if row["hidden"].as_bool().unwrap_or(false) {
            return;
        }
        let Some(id) = row["id"].as_str().or_else(|| row["model"].as_str()).map(str::trim)
            .filter(|id| !id.is_empty()) else { return };
        if !self.ids.iter().any(|known| known == id) {
            self.ids.push(id.to_owned());
        }
        let mut supported = Vec::new();
        for effort in row["supportedReasoningEfforts"].as_array().into_iter().flatten()
            .filter_map(|entry| entry["reasoningEffort"].as_str())
            .map(str::trim).filter(|effort| !effort.is_empty())
        {
            if !supported.iter().any(|known| known == effort) {
                supported.push(effort.to_owned());
            }
        }
        let default = row["defaultReasoningEffort"].as_str().map(str::trim)
            .filter(|effort| !effort.is_empty()).map(str::to_owned);
        if row["supportedReasoningEfforts"].is_array() || default.is_some() {
            self.efforts.entry(id.to_owned()).or_insert(ModelEffort { supported, default });
        }
    }

    fn add_claude(&mut self, row: &Value) {
        if row["disabled"].as_bool() == Some(true) {
            return;
        }
        // Merge aliases into the same catalog entry as their resolved model.
        let id = row["resolvedModel"].as_str().map(str::trim).filter(|id| !id.is_empty())
            .or_else(|| row["value"].as_str().map(str::trim));
        let Some(id) = id.filter(|id| !id.is_empty() && *id != "default") else { return };
        if !self.ids.iter().any(|known| known == id) {
            self.ids.push(id.to_owned());
        }
        if row["supportsEffort"].as_bool() == Some(false) {
            return;
        }
        let Some(levels) = row["supportedEffortLevels"].as_array() else { return };
        let metadata = self.efforts.entry(id.to_owned()).or_insert_with(|| ModelEffort {
            supported: Vec::new(),
            default: None,
        });
        for level in levels.iter().filter_map(Value::as_str).map(str::trim).filter(|level| !level.is_empty()) {
            if !metadata.supported.iter().any(|known| known == level) {
                metadata.supported.push(level.to_owned());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ModelCatalog;
    use serde_json::json;

    #[test]
    fn codex_model_efforts_are_saved_by_model_id() {
        let mut catalog = ModelCatalog::default();
        catalog.add_codex(&json!({ "id": "gpt-6.2-sol" }));
        catalog.add_codex(&json!({
            "id": "gpt-6.2-sol",
            "supportedReasoningEfforts": [
                { "reasoningEffort": "medium" },
                { "reasoningEffort": "max" },
                { "reasoningEffort": "max" }
            ],
            "defaultReasoningEffort": "medium"
        }));
        catalog.add_codex(&json!({ "model": "legacy" }));
        catalog.add_codex(&json!({ "id": "hidden", "hidden": true, "defaultReasoningEffort": "high" }));

        assert_eq!(catalog.ids, ["gpt-6.2-sol", "legacy"]);
        assert_eq!(serde_json::to_value(&catalog.efforts).unwrap(), json!({
            "gpt-6.2-sol": { "supported": ["medium", "max"], "default": "medium" }
        }));
    }

    #[test]
    fn claude_model_efforts_follow_the_resolved_id() {
        let mut catalog = ModelCatalog::default();
        catalog.add_claude(&json!({ "value": "claude-opus-5[1m]" }));
        catalog.add_claude(&json!({
            "value": "opus[1m]", "resolvedModel": "claude-opus-5[1m]",
            "supportsEffort": true, "supportedEffortLevels": ["low", "max", "max"]
        }));
        catalog.add_claude(&json!({ "value": "haiku" }));
        catalog.add_claude(&json!({ "value": "disabled", "disabled": true }));
        assert_eq!(catalog.ids, ["claude-opus-5[1m]", "haiku"]);
        assert_eq!(catalog.efforts.len(), 1);
        let effort = &catalog.efforts["claude-opus-5[1m]"];
        assert_eq!(effort.supported, ["low", "max"]);
        assert_eq!(effort.default, None);
    }
}

/// Visible models and reasoning effort options from the signed-in Codex app-server.
async fn codex_models(
    state: &AppState,
    provider: &crate::config::ProviderConfig,
) -> ApiResult<ModelCatalog> {
    let mut client = super::codex::new_client(state, provider).await?;
    let listed = list_codex_models(&mut client).await;
    client.close().await;
    let models = listed.map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;
    if models.ids.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Codex không trả model nào; hãy đăng nhập Codex rồi thử lại",
        ));
    }
    Ok(models)
}

async fn list_codex_models(client: &mut crate::providers::codex::transport::Client) -> anyhow::Result<ModelCatalog> {
    let mut models = ModelCatalog::default();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let mut params = serde_json::json!({"limit": 100, "includeHidden": false});
        if let Some(cursor) = &cursor {
            params["cursor"] = Value::String(cursor.clone());
        }
        let page = client.call("model/list", params).await?;
        for row in page["data"].as_array().into_iter().flatten() {
            models.add_codex(row);
        }
        cursor = page["nextCursor"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(models)
}

/// Model ids from Claude Code: one `list_models` control request over `-p` stream-json (no API turn), like the SDK's `supportedModels()`.
async fn claude_models(
    state: &AppState,
    provider: &crate::config::ProviderConfig,
) -> ApiResult<ModelCatalog> {
    let bad_gateway = |error: anyhow::Error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string());
    let executable = providers::cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Claude CLI trên máy"))?;
    let home = providers::claude::environment::home(&state.config_path, provider).map_err(bad_gateway)?;
    let models = list_claude_models(&executable, &home).await.map_err(bad_gateway)?;
    if models.ids.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Claude không trả model nào; hãy đăng nhập Claude rồi thử lại",
        ));
    }
    Ok(models)
}

async fn list_claude_models(executable: &FsPath, home: &FsPath) -> anyhow::Result<ModelCatalog> {
    use anyhow::Context as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let args = ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"]
        .map(OsString::from);
    let mut command = providers::cli_support::command_for(executable, &args);
    providers::claude::environment::apply(&mut command, home);
    let mut child = command
        .current_dir(home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("Không khởi động được Claude CLI")?;
    let mut stdin = child.stdin.take().context("Không mở được kết nối Claude")?;
    let stdout = child.stdout.take().context("Không đọc được kết nối Claude")?;
    let request = serde_json::json!({
        "type": "control_request",
        "request_id": "mutis-list-models",
        "request": {"subtype": "list_models"}
    });
    stdin.write_all(format!("{request}\n").as_bytes()).await?;
    stdin.flush().await?;
    let read = async {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
            if value["type"] != "control_response" {
                continue;
            }
            let response = &value["response"];
            if response["subtype"] == "error" {
                anyhow::bail!("Claude CLI không hỗ trợ lấy danh sách model; hãy cập nhật Claude CLI");
            }
            if let Some(rows) = response["response"]["models"].as_array() {
                let mut models = ModelCatalog::default();
                for row in rows {
                    models.add_claude(row);
                }
                return Ok(models);
            }
        }
        anyhow::bail!("Claude CLI đã đóng kết nối trước khi trả danh sách model")
    };
    let result = tokio::time::timeout(Duration::from_secs(30), read)
        .await
        .map_err(|_| anyhow::anyhow!("Claude CLI không phản hồi trong 30 giây"))?;
    drop(stdin);
    let _ = child.kill().await;
    result
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
    if matches!(provider.kind.as_str(), "codex_cli" | "claude_cli" | "antigravity_cli") {
        let (models, efforts) = if provider.kind == "claude_cli" {
            let catalog = claude_models(&state, &provider).await?;
            (catalog.ids, catalog.efforts)
        } else if provider.kind == "antigravity_cli" {
            (super::antigravity::list_models(&provider).await?, BTreeMap::new())
        } else {
            let catalog = codex_models(&state, &provider).await?;
            (catalog.ids, catalog.efforts)
        };
        let mut config = state.config.write().await;
        let mut next = config.clone();
        let entry = next.providers.iter_mut().find(|entry| entry.id == id)
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
        if (entry.kind.as_str(), entry.executable.as_str(), entry.web_login)
            != (provider.kind.as_str(), provider.executable.as_str(), provider.web_login)
        {
            return Err(ApiError::new(StatusCode::CONFLICT, "Cấu hình nguồn đã thay đổi. Hãy tải lại danh sách model."));
        }
        entry.models = models.clone();
        entry.model_efforts = efforts;
        persist_config(&state, &next)?;
        *config = next;
        return Ok(Json(ModelsResponse { models }));
    }
    if provider.kind != "openai_compat" {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Chỉ hỗ trợ tải model cho Codex CLI, Claude Code CLI và OpenAI-compatible",
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
