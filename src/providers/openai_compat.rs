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

pub struct OpenAiCompatibleProvider;

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if request.provider.api_key.trim().is_empty() {
            bail!("API key chưa được cấu hình cho nhà cung cấp này");
        }
        let base_url = request.provider.base_url.trim().trim_end_matches('/');
        if base_url.is_empty() {
            bail!("URL cơ sở chưa được cấu hình cho nhà cung cấp này");
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
        let messages: Vec<Value> = source_messages
            .iter()
            .map(openai_message)
            .collect::<Result<Vec<_>>>()?;
        let mut body = json!({
            "model": request.model,
            "messages": messages,
            "stream": true
        });
        if request.effort != Effort::Default {
            body["reasoning_effort"] = json!(match request.effort {
                Effort::Low => "low",
                Effort::Medium => "medium",
                Effort::High | Effort::Xhigh | Effort::Max => "high",
                Effort::Default => unreachable!(),
            });
        }
        let client = reqwest::Client::new();
        let secret = request.provider.api_key.clone();
        let mut response = match super::send_with_retry(
            &client,
            &format!("{base_url}/chat/completions"),
            vec![(
                "Authorization".into(),
                format!("Bearer {}", request.provider.api_key),
            )],
            &body,
            &request.mutis,
            &events,
            &cancel,
        )
        .await
        {
            Ok(response) => response,
            Err(_error) if cancel.is_cancelled() => return Ok(()),
            Err(error) => return Err(error.context("Không thể kết nối tới API tương thích OpenAI")),
        };
        if response.status() == reqwest::StatusCode::BAD_REQUEST
            && body.get("reasoning_effort").is_some()
        {
            let response_body = response.text().await.unwrap_or_default();
            if !source_messages
                .iter()
                .all(|message| message.attachments.is_empty())
            {
                let detail = response_body.to_ascii_lowercase();
                if ["image", "vision", "content"]
                    .iter()
                    .any(|word| detail.contains(word))
                {
                    bail!("Model này không hỗ trợ ảnh");
                }
            }
            if response_body
                .to_ascii_lowercase()
                .contains("reasoning_effort")
            {
                emit_text(
                    &events,
                    "log",
                    "Model không hỗ trợ mức suy luận, đã dùng mặc định".to_owned(),
                )
                .await?;
                body.as_object_mut()
                    .expect("object body")
                    .remove("reasoning_effort");
                response = super::send_with_retry(
                    &client,
                    &format!("{base_url}/chat/completions"),
                    vec![(
                        "Authorization".into(),
                        format!("Bearer {}", request.provider.api_key),
                    )],
                    &body,
                    &request.mutis,
                    &events,
                    &cancel,
                )
                .await?;
            } else {
                bail!(
                    "OpenAI-compatible API returned HTTP 400: {}",
                    safe_error_body(&response_body, &secret)
                );
            }
        }
        if !response.status().is_success() {
            let status = response.status();
            let response_body = response.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::BAD_REQUEST
                && !source_messages.iter().all(|m| m.attachments.is_empty())
            {
                let detail = response_body.to_ascii_lowercase();
                if ["image", "vision", "content"]
                    .iter()
                    .any(|word| detail.contains(word))
                {
                    bail!("Model này không hỗ trợ ảnh");
                }
            }
            bail!(
                "API tương thích OpenAI trả về HTTP {status}: {}",
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
            let chunk = chunk.context("Lỗi khi nhận dữ liệu từ API tương thích OpenAI")?;
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

fn openai_message(message: &crate::chat::ChatMessage) -> Result<Value> {
    if message.attachments.is_empty() {
        return Ok(json!({"role":message.role,"content":message.content}));
    }
    let mut content = vec![json!({"type":"text","text":message.content})];
    for attachment in &message.attachments {
        let bytes = std::fs::read(&attachment.path).context("Không thể đọc ảnh đính kèm")?;
        content.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}", attachment.mime, crate::api::attachments::encode_base64(&bytes))}}));
    }
    Ok(json!({"role":message.role,"content":content}))
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
    let data = data.trim();
    if data.is_empty() || data == "[DONE]" {
        return Ok(());
    }
    let Ok(value) = serde_json::from_str::<Value>(data) else {
        return Ok(());
    };
    if let Some(error) = error_value_text(&value) {
        emit_text(events, "error", safe_error_body(&error, secret)).await?;
        return Ok(());
    }
    let delta = &value["choices"][0]["delta"];
    if let Some(content) = delta["content"].as_str() {
        emit_text(events, "delta", content.to_owned()).await?;
    }
    if let Some(reasoning) = delta["reasoning_content"].as_str() {
        emit_text(events, "reasoning", reasoning.to_owned()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod image_builder_tests {
    use super::openai_message;
    use crate::chat::{ChatAttachment, ChatMessage};
    use std::fs;
    use uuid::Uuid;
    #[test]
    fn openai_builder_adds_data_image_after_text() {
        let path = std::env::temp_dir().join(format!("{}.png", Uuid::new_v4()));
        fs::write(&path, b"pixels").unwrap();
        let message = ChatMessage {
            debate_id: None,
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
        let value = openai_message(&message).unwrap();
        assert_eq!(value["content"][0]["type"], "text");
        assert_eq!(
            value["content"][1]["image_url"]["url"],
            "data:image/png;base64,cGl4ZWxz"
        );
        let _ = fs::remove_file(path);
    }
}
