pub(crate) mod agent;
pub mod anthropic;
pub mod claude_cli;
pub(crate) mod cli_support;
pub mod codex_cli;
pub(crate) mod codex;
pub mod deepseek_agent;
pub(crate) mod discovery;
pub(crate) mod limits;
pub mod mcp;
pub mod openai_compat;

use crate::{
    chat::{ChatMessage, Effort, Permission},
    config::{MutisConfig, ProviderConfig},
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ProviderRequest {
    pub provider: ProviderConfig,
    pub model: String,
    pub folder: String,
    pub permission: Permission,
    pub effort: Effort,
    pub messages: Vec<ChatMessage>,
    pub session_id: Option<String>,
    pub mutis: MutisConfig,
    /// App-managed MCP servers for API agents and the isolated Codex adapter.
    pub mcp: Vec<crate::config::McpServerConfig>,
}

pub(crate) fn is_deepseek_flash(model: &str) -> bool {
    let name: String = model.to_ascii_lowercase().chars()
        .filter(|c| !matches!(c, '-' | '_' | '.') && !c.is_whitespace()).collect();
    name.contains("deepseekv41flash") || name == "deepseekflash"
}

pub(crate) fn api_reasoning_effort(model: &str, effort: Effort) -> Option<&'static str> {
    if is_deepseek_flash(model) {
        return Some(match effort {
            Effort::Low => "low",
            Effort::Max => "max",
            _ => "high",
        });
    }
    match effort {
        Effort::Default => None,
        Effort::Low => Some("low"),
        Effort::Medium => Some("medium"),
        _ => Some("high"),
    }
}

pub(crate) fn api_images_with_limit(messages: &mut [ChatMessage]) -> bool {
    let mut total = messages
        .iter()
        .flat_map(|m| &m.attachments)
        .filter_map(|a| fs::metadata(&a.path).ok())
        .map(|m| m.len().div_ceil(3).saturating_mul(4))
        .sum::<u64>();
    let mut dropped = false;
    for message in messages.iter_mut().filter(|m| m.role == "user") {
        if total < 15 * 1024 * 1024 {
            break;
        }
        while total >= 15 * 1024 * 1024 && !message.attachments.is_empty() {
            let attachment = message.attachments.remove(0);
            total = total.saturating_sub(
                fs::metadata(attachment.path)
                    .map(|m| m.len().div_ceil(3).saturating_mul(4))
                    .unwrap_or(0),
            );
            dropped = true;
        }
    }
    dropped
}

pub(crate) fn image_block(attachment: &crate::chat::ChatAttachment) -> Result<Value> {
    let bytes = fs::read(&attachment.path).context("Không thể đọc ảnh đính kèm")?;
    Ok(
        json!({"type":"image","source":{"type":"base64","media_type":attachment.mime,"data":crate::api::attachments::encode_base64(&bytes)}}),
    )
}

pub(crate) fn latest_chat_attachment_dir(messages: &[ChatMessage]) -> Option<PathBuf> {
    messages
        .iter()
        .rev()
        .find(|m| m.role == "user" && !m.attachments.is_empty())?
        .attachments
        .first()?
        .path
        .parse::<PathBuf>()
        .ok()?
        .parent()
        .map(|path| crate::core::text::strip_verbatim(path.to_path_buf()))
}

pub(crate) fn latest_image_paths(
    messages: &[ChatMessage],
    expected_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let expected_dir = fs::canonicalize(expected_dir).context("Thư mục ảnh chat không hợp lệ")?;
    let Some(message) = messages
        .iter()
        .rev()
        .find(|m| m.role == "user" && !m.attachments.is_empty())
    else {
        return Ok(Vec::new());
    };
    message
        .attachments
        .iter()
        .map(|a| {
            let path = fs::canonicalize(&a.path).context("Không thể giải quyết đường dẫn ảnh")?;
            if !path.starts_with(&expected_dir) || !path.is_file() {
                anyhow::bail!("Đường dẫn ảnh nằm ngoài thư mục đính kèm chat");
            }
            Ok(crate::core::text::strip_verbatim(path))
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ProviderEvent {
    pub name: &'static str,
    pub data: Value,
}

impl ProviderEvent {
    pub fn text(name: &'static str, text: impl Into<String>) -> Self {
        Self {
            name,
            data: json!({ "text": text.into() }),
        }
    }
}

#[async_trait]
pub trait Provider: Send + Sync {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()>;
}

pub fn for_kind(kind: &str) -> Result<Box<dyn Provider>> {
    match kind {
        "openai_compat" => Ok(Box::new(openai_compat::OpenAiCompatibleProvider)),
        "deepseek_agent" | "mcp_agent" => Ok(Box::new(deepseek_agent::DeepSeekAgentProvider)),
        "anthropic" => Ok(Box::new(anthropic::AnthropicProvider)),
        "codex_cli" => Ok(Box::new(codex::CodexProvider)),
        "claude_cli" => Ok(Box::new(claude_cli::ClaudeCliProvider)),
        _ => bail!("Loại nhà cung cấp không được hỗ trợ: {kind}"),
    }
}

pub fn retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
}

pub fn retry_delay(base_ms: u64, retry_index: u32, jitter_ms: u64) -> std::time::Duration {
    std::time::Duration::from_millis(
        base_ms
            .saturating_mul(1u64.checked_shl(retry_index).unwrap_or(u64::MAX))
            .saturating_add(jitter_ms.min(250)),
    )
}

pub async fn send_with_retry(
    client: &reqwest::Client,
    url: &str,
    headers: Vec<(String, String)>,
    body: &Value,
    config: &MutisConfig,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> Result<reqwest::Response> {
    let max_retries = if config.effective(config.retry_api) {
        config.retry_attempts
    } else {
        0
    };
    let mut retry = 0;
    loop {
        let mut builder = client.post(url).json(body);
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        let result = tokio::select! { _ = cancel.cancelled() => bail!("cancelled"), result = builder.send() => result };
        let retryable = match &result {
            Err(error) => error.is_connect() || error.is_timeout(),
            Ok(response) => retryable_status(response.status()),
        };
        if !retryable || retry >= max_retries {
            return result.context("Không thể gửi yêu cầu API");
        }
        let reason = match &result {
            Err(error) => error.to_string(),
            Ok(response) => format!("HTTP {}", response.status()),
        };
        retry += 1;
        let detail = format!("Thử lại lần {retry}/{max_retries} sau lỗi {reason}");
        emit_text(events, "log", detail.clone()).await?;
        emit_text(events, "retry", detail).await?;
        let jitter = crate::mutis::now()
            .wrapping_mul(31)
            .wrapping_add(retry as u64 * 73)
            % 251;
        tokio::select! { _ = cancel.cancelled() => bail!("cancelled"), _ = tokio::time::sleep(retry_delay(config.retry_base_ms, retry - 1, jitter)) => {} }
    }
}

pub(crate) async fn emit_text(
    events: &mpsc::Sender<ProviderEvent>,
    name: &'static str,
    text: String,
) -> Result<()> {
    events
        .send(ProviderEvent::text(name, text))
        .await
        .map_err(|_| anyhow::anyhow!("Chat stream closed"))
}

pub(crate) fn error_value_text(value: &Value) -> Option<String> {
    let error = value.get("error")?;
    if let Some(message) = error.as_str() {
        return Some(message.to_owned());
    }
    if let Some(message) = error.get("message").and_then(Value::as_str) {
        return Some(message.to_owned());
    }
    Some(error.to_string())
}

pub(crate) fn safe_error_body(body: &str, secret: &str) -> String {
    let mut body = if let Ok(mut value) = serde_json::from_str::<Value>(body) {
        redact_sensitive_fields(&mut value);
        value.to_string()
    } else {
        body.to_owned()
    };
    if !secret.is_empty() {
        body = body.replace(secret, "[redacted]");
    }
    let truncated = body.chars().take(500).collect::<String>();
    if truncated.trim().is_empty() {
        "(empty response body)".to_owned()
    } else {
        truncated
    }
}

fn redact_sensitive_fields(value: &mut Value) {
    match value {
        Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                let key = key.to_ascii_lowercase();
                if key.contains("key")
                    || key.contains("token")
                    || key.contains("secret")
                    || key.contains("authorization")
                {
                    *value = Value::String("[redacted]".to_owned());
                } else {
                    redact_sensitive_fields(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_sensitive_fields(value);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod retry_tests {
    use super::{retry_delay, retryable_status};
    #[test]
    fn deepseek_flash_effort_preserves_three_levels() {
        use super::{api_reasoning_effort, Effort};
        for model in ["DeepSeek-V4.1-Flash", "deepseek-v4.1-flash", "deepseek-flash"] {
            for (effort, expected) in [(Effort::Low, "low"), (Effort::High, "high"), (Effort::Max, "max"),
                (Effort::Default, "high"), (Effort::Medium, "high"), (Effort::Xhigh, "high")] {
                assert_eq!(api_reasoning_effort(model, effort), Some(expected));
            }
        }
        assert_eq!(api_reasoning_effort("other-model", Effort::Default), None);
        assert_eq!(api_reasoning_effort("other-model", Effort::Medium), Some("medium"));
        assert_eq!(api_reasoning_effort("other-model", Effort::Max), Some("high"));
    }
    #[test]
    fn retries_only_transient_statuses() {
        for status in [429, 500, 502, 503, 504] {
            assert!(retryable_status(
                reqwest::StatusCode::from_u16(status).unwrap()
            ));
        }
        for status in [400, 401, 404, 501] {
            assert!(!retryable_status(
                reqwest::StatusCode::from_u16(status).unwrap()
            ));
        }
    }
    #[test]
    fn backoff_doubles_and_caps_jitter() {
        assert_eq!(retry_delay(2000, 0, 0).as_millis(), 2000);
        assert_eq!(retry_delay(2000, 1, 250).as_millis(), 4250);
        assert_eq!(retry_delay(2000, 2, 999).as_millis(), 8250);
    }
}
