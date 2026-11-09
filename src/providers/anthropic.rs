use super::{
    emit_text, error_value_text, safe_error_body, Provider, ProviderEvent, ProviderRequest,
};
use crate::chat::Effort;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct AnthropicProvider;

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
        let (system, messages): (Vec<String>, Vec<Value>) = request
            .messages
            .iter()
            .filter(|message| !message.content.is_empty())
            .fold(
                (Vec::new(), Vec::new()),
                |(mut system, mut messages), message| {
                    if message.role == "system" {
                        system.push(message.content.clone());
                    } else {
                        messages.push(json!({"role": message.role, "content": message.content}));
                    }
                    (system, messages)
                },
            );
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": 8192,
            "stream": true
        });
        if request.effort != Effort::Default {
            let budget = match request.effort {
                Effort::Low => 2048,
                Effort::Medium => 8192,
                Effort::High => 16000,
                Effort::Xhigh | Effort::Max => 32000,
                Effort::Default => 0,
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
            "https://api.anthropic.com/v1/messages",
            vec![
                ("x-api-key".into(), request.provider.api_key.clone()),
                ("anthropic-version".into(), "2023-06-01".into()),
            ],
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

async fn process_line(
    line: &[u8],
    events: &mpsc::Sender<ProviderEvent>,
    secret: &str,
) -> Result<()> {
    let line = String::from_utf8_lossy(line)
        .trim()
        .trim_end_matches('\r')
        .to_owned();
    let Some(data) = line.strip_prefix("data:") else {
        return Ok(());
    };
    let Ok(value) = serde_json::from_str::<Value>(data.trim()) else {
        return Ok(());
    };
    if let Some(error) = error_value_text(&value) {
        emit_text(events, "error", safe_error_body(&error, secret)).await?;
        return Ok(());
    }
    if value["type"] == "content_block_delta" && value["delta"]["type"] == "text_delta" {
        if let Some(text) = value["delta"]["text"].as_str() {
            emit_text(events, "delta", text.to_owned()).await?;
        }
    }
    if value["type"] == "content_block_delta" && value["delta"]["type"] == "thinking_delta" {
        if let Some(text) = value["delta"]["thinking"].as_str() {
            emit_text(events, "reasoning", text.to_owned()).await?;
        }
    }
    Ok(())
}
