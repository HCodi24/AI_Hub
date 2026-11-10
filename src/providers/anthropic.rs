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

fn anthropic_content(message: &crate::chat::ChatMessage) -> Result<Vec<Value>> {
    let mut blocks = Vec::new();
    for attachment in &message.attachments {
        blocks.push(super::image_block(attachment)?);
    }
    if !message.content.is_empty() {
        blocks.push(json!({"type":"text","text":message.content}));
    }
    Ok(blocks)
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

#[cfg(test)]
mod image_builder_tests {
    use super::anthropic_content;
    use crate::chat::{ChatAttachment, ChatMessage};
    use std::fs;
    use uuid::Uuid;
    #[test]
    fn anthropic_builder_places_image_before_text() {
        let path = std::env::temp_dir().join(format!("{}.png", Uuid::new_v4()));
        fs::write(&path, b"pixels").unwrap();
        let message = ChatMessage {
            role: "user".into(),
            content: "Look".into(),
            attachments: vec![ChatAttachment {
                id: "id".into(),
                mime: "image/png".into(),
                path: path.to_string_lossy().into_owned(),
            }],
            reasoning: String::new(),
            logs: vec![],
            diff: String::new(),
            error: None,
        };
        let blocks = anthropic_content(&message).unwrap();
        assert_eq!(blocks[0]["type"], "image");
        assert_eq!(blocks[0]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["type"], "text");
        let _ = fs::remove_file(path);
    }
}
