pub mod claude_cli;
pub mod codex_cli;
pub mod anthropic;
pub(crate) mod cli_support;
pub mod openai_compat;

use crate::{chat::ChatMessage, config::ProviderConfig};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ProviderRequest {
    pub provider: ProviderConfig,
    pub model: String,
    pub folder: String,
    pub messages: Vec<ChatMessage>,
    pub session_id: Option<String>,
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
        "anthropic" => Ok(Box::new(anthropic::AnthropicProvider)),
        "codex_cli" => Ok(Box::new(codex_cli::CodexCliProvider)),
        "claude_cli" => Ok(Box::new(claude_cli::ClaudeCliProvider)),
        _ => bail!("Loại nhà cung cấp không được hỗ trợ: {kind}"),
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
