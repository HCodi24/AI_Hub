use super::{emit_text, Provider, ProviderEvent, ProviderRequest};
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
        let messages: Vec<Value> = request
            .messages
            .iter()
            .map(|message| json!({"role": message.role, "content": message.content}))
            .collect();
        let body = json!({
            "model": request.model,
            "messages": messages,
            "stream": true
        });
        let client = reqwest::Client::new();
        let response = tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            response = client
                .post(format!("{base_url}/chat/completions"))
                .bearer_auth(&request.provider.api_key)
                .json(&body)
                .send() => response.context("Không thể kết nối tới API tương thích OpenAI")?,
        };
        if !response.status().is_success() {
            bail!("API tương thích OpenAI trả về HTTP {}", response.status());
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
                process_line(&line, &events).await?;
            }
        }
        if !pending.is_empty() {
            process_line(&pending, &events).await?;
        }
        Ok(())
    }
}

async fn process_line(line: &[u8], events: &mpsc::Sender<ProviderEvent>) -> Result<()> {
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
    let delta = &value["choices"][0]["delta"];
    if let Some(content) = delta["content"].as_str() {
        emit_text(events, "delta", content.to_owned()).await?;
    }
    if let Some(reasoning) = delta["reasoning_content"].as_str() {
        emit_text(events, "reasoning", reasoning.to_owned()).await?;
    }
    Ok(())
}
