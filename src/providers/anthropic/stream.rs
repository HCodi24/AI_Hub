use crate::providers::{emit_text, error_value_text, safe_error_body, ProviderEvent};
use anyhow::Result;
use serde_json::Value;
use tokio::sync::mpsc;

pub(super) async fn process_line(
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
