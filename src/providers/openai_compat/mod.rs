use super::{emit_text, safe_error_body, Provider, ProviderEvent, ProviderRequest};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod message;
mod stream;
pub(crate) use message::openai_message;
use stream::process_line;

pub struct OpenAiCompatibleProvider;

// Write and Full behave identically here; command execution is only provided by CLI sources.

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
        if let Some(effort) = super::api_reasoning_effort(&request.model, request.effort) {
            body["reasoning_effort"] = json!(effort);
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
            if images_rejected(&source_messages, &response_body) {
                bail!("Model này không hỗ trợ ảnh");
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
                && images_rejected(&source_messages, &response_body)
            {
                bail!("Model này không hỗ trợ ảnh");
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

fn images_rejected(messages: &[crate::chat::ChatMessage], body: &str) -> bool {
    if messages
        .iter()
        .all(|message| message.attachments.is_empty())
    {
        return false;
    }
    let detail = body.to_ascii_lowercase();
    ["image", "vision", "content"]
        .iter()
        .any(|word| detail.contains(word))
}

#[cfg(test)]
mod tests;
