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
