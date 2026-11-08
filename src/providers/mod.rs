pub mod anthropic;
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
        "codex_cli" | "claude_cli" => Ok(Box::new(UnsupportedCliProvider)),
        _ => bail!("Loại nhà cung cấp không được hỗ trợ: {kind}"),
    }
}

struct UnsupportedCliProvider;

#[async_trait]
impl Provider for UnsupportedCliProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        _cancel: CancellationToken,
    ) -> Result<()> {
        let _cli_context = (request.folder, request.session_id);
        let _ = events
            .send(ProviderEvent::text("error", "Chưa hỗ trợ ở bước 1"))
            .await;
        Ok(())
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
