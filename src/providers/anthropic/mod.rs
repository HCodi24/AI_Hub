use super::agent::AgentTools;
use super::{emit_text, safe_error_body, Provider, ProviderEvent, ProviderRequest};
use crate::chat::Effort;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::json;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod message;
mod stream;
mod tools;
use message::anthropic_content;
use stream::process_line;
use tools::run_with_tools;

pub struct AnthropicProvider;

pub(super) const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";

pub(super) fn auth_headers(api_key: &str) -> Vec<(String, String)> {
    vec![
        ("x-api-key".into(), api_key.to_owned()),
        ("anthropic-version".into(), "2023-06-01".into()),
    ]
}

const AGENT_READ: &str = "You are a coding assistant with READ-ONLY file tools (list and read). Work only in the supplied folder. You cannot edit files or run commands. Explore as needed and answer concisely.";
const AGENT_WRITE: &str = "You are a coding assistant with file tools. Work only in the supplied folder with the tools. Do not run commands. Make only the changes the user asked for, then give a concise report.";
const AGENT_FULL: &str = "You are a coding assistant with file tools. Work only in the supplied folder with the tools. You can run commands in the working folder with run_command; prefer small, safe commands; never run destructive commands the user did not ask for. Make only the changes the user asked for, then give a concise report.";

#[async_trait]
impl Provider for AnthropicProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if request.provider.api_key.trim().is_empty() {
            bail!("API key chưa được cấu hình cho nhà cung cấp này");
        }
        let mut source_messages = request.messages.clone();
        if super::api_images_with_limit(&mut source_messages) {
            emit_text(
                &events,
                "log",
                "Bỏ ảnh cũ vì tổng dung lượng ảnh vượt 15 MB".to_owned(),
            )
            .await?;
        }
        let mut system = Vec::new();
        let mut messages = Vec::new();
        for message in source_messages
            .iter()
            .filter(|message| !message.content.is_empty() || !message.attachments.is_empty())
        {
            if message.role == "system" {
                system.push(message.content.clone());
                continue;
            }
            let blocks = anthropic_content(message)?;
            messages.push(json!({"role": message.role, "content": blocks}));
        }
        // The handler marks this request-local copy for Anthropic file-agent mode.
        let files = request.provider.kind == "anthropic" && request.provider.agent_capable;
        if files {
            system.push(
                match request.permission {
                    crate::chat::Permission::Read => AGENT_READ,
                    crate::chat::Permission::Write => AGENT_WRITE,
                    crate::chat::Permission::Full => AGENT_FULL,
                }
                .to_owned(),
            );
        }
        if files || !request.mcp.is_empty() {
            let mcp = super::mcp::McpTools::start(
                &request.mcp,
                &request.folder,
                request.permission != crate::chat::Permission::Read,
                &events,
            )
            .await;
            let root = if files {
                let root = PathBuf::from(&request.folder)
                    .canonicalize()
                    .context("Thư mục làm việc không tồn tại")?;
                if !root.is_dir() {
                    bail!("Thư mục làm việc không hợp lệ");
                }
                root
            } else {
                PathBuf::from(&request.folder)
            };
            let tools = AgentTools::new(
                root,
                request.permission,
                request.mutis.deepseek_max_file_lines,
                files,
                mcp,
            );
            if !tools.anthropic_definitions().is_empty() {
                return run_with_tools(
                    &request,
                    system.join("\n\n"),
                    messages,
                    tools,
                    &events,
                    &cancel,
                )
                .await;
            }
        }
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": 8192,
            "stream": true
        });
        if !matches!(request.effort, Effort::Default | Effort::None) {
            let budget = match request.effort {
                Effort::Low | Effort::Minimal => 2048,
                Effort::Medium => 8192,
                Effort::High => 16000,
                Effort::Xhigh | Effort::Max | Effort::Ultra => 32000,
                Effort::Default | Effort::None => 0,
            };
            body["thinking"] = json!({"type":"enabled", "budget_tokens": budget});
            body["max_tokens"] = json!(budget + 8192);
        }
        if !system.is_empty() {
            body["system"] = json!(system.join("\n\n"));
        }
        let client = reqwest::Client::new();
        let secret = request.provider.api_key.clone();
        let response = match super::send_with_retry(
            &client,
            MESSAGES_URL,
            auth_headers(&request.provider.api_key),
            &body,
            &request.mutis,
            &events,
            &cancel,
        )
        .await
        {
            Ok(response) => response,
            Err(_) if cancel.is_cancelled() => return Ok(()),
            Err(error) => return Err(error.context("Không thể kết nối tới Anthropic API")),
        };
        if !response.status().is_success() {
            let status = response.status();
            let response_body = response.text().await.unwrap_or_default();
            bail!(
                "Anthropic API trả về HTTP {status}: {}",
                safe_error_body(&response_body, &secret)
            );
        }

        let mut stream = response.bytes_stream();
        let mut pending = Vec::new();
        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = chunk else { break };
            let chunk = chunk.context("Lỗi khi nhận dữ liệu từ Anthropic API")?;
            pending.extend_from_slice(&chunk);
            while let Some(newline) = pending.iter().position(|byte| *byte == b'\n') {
                let line: Vec<u8> = pending.drain(..=newline).collect();
                process_line(&line, &events, &secret).await?;
            }
        }
        if !pending.is_empty() {
            process_line(&pending, &events, &secret).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
