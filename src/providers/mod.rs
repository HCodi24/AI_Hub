pub(crate) mod agent;
pub mod anthropic;
pub(crate) mod approvals;
pub mod antigravity_cli;
pub mod claude_cli;
pub(crate) mod claude;
pub(crate) mod cli_support;
pub mod codex_cli;
pub(crate) mod codex;
pub mod deepseek_agent;
pub(crate) mod delegate;
pub(crate) mod schedule;
pub(crate) mod worker_session;
pub(crate) mod discovery;
pub(crate) mod limits;
pub mod mcp;
pub mod openai_compat;
pub mod quota;

mod errors;
mod images;
mod retry;

pub(crate) use errors::{error_value_text, safe_error_body};
pub(crate) use images::{
    api_images_with_limit, image_block, latest_chat_attachment_dir, latest_image_paths,
};
pub use retry::{retry_delay, retryable_status, send_with_retry};

#[cfg(test)]
mod tests;

use crate::{
    chat::{ChatMessage, Effort, Permission},
    config::{MutisConfig, ProviderConfig},
};
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
    pub permission: Permission,
    pub effort: Effort,
    pub messages: Vec<ChatMessage>,
    pub session_id: Option<String>,
    pub mutis: MutisConfig,
    /// App-managed MCP servers for API agents and the isolated Codex adapter.
    pub mcp: Vec<crate::config::McpServerConfig>,
    /// When not empty, the file-tool agent may only edit these files (or create new files next to them).
    pub write_scope: Vec<String>,
    /// Native thread of the independent checker (see `ChatRecord::checker_session_id`); None = start a new one.
    pub checker_session_id: Option<String>,
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
        "codex_cli" => Ok(Box::new(codex_cli::CodexCliProvider)),
        "claude_cli" => Ok(Box::new(claude_cli::ClaudeCliProvider)),
        "antigravity_cli" => Ok(Box::new(antigravity_cli::AntigravityCliProvider)),
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
