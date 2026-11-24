use super::{auth_headers, MESSAGES_URL};
use crate::providers::agent::AgentTools;
use crate::providers::{emit_text, safe_error_body, ProviderEvent, ProviderRequest};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Tool loop for file and MCP tools. Extended thinking is not used here (its blocks would have to be echoed back with tool results).
pub(super) async fn run_with_tools(
    request: &ProviderRequest,
    system: String,
    mut messages: Vec<Value>,
    mut tools: AgentTools,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .build()?;
    let secret = request.provider.api_key.clone();
    let definitions = tools.anthropic_definitions();
    let (mut tokens_in, mut tokens_out) = (0u64, 0u64);
    let mut answer = String::new();
    for _ in 0..request.mutis.deepseek_max_steps.clamp(5, 40) {
        let mut body = json!({"model": request.model, "messages": messages, "max_tokens": 8192, "tools": definitions});
        if !system.is_empty() {
            body["system"] = json!(system);
        }
        let response = match crate::providers::send_with_retry(
            &client,
            MESSAGES_URL,
            auth_headers(&secret),
            &body,
            &request.mutis,
            events,
            cancel,
        )
        .await
        {
            Ok(response) => response,
            Err(_) if cancel.is_cancelled() => return Ok(()),
            Err(error) => return Err(error.context("Không thể kết nối tới Anthropic API")),
        };
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            bail!(
                "Anthropic API trả về HTTP {status}: {}",
                safe_error_body(&text, &secret)
            );
        }
        let value: Value = tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            value = response.json::<Value>() => value.context("Phản hồi Anthropic không hợp lệ")?,
        };
        tokens_in += value["usage"]["input_tokens"].as_u64().unwrap_or(0);
        tokens_out += value["usage"]["output_tokens"].as_u64().unwrap_or(0);
        let blocks = value["content"].as_array().cloned().unwrap_or_default();
        for block in blocks.iter().filter(|b| b["type"] == "text") {
            answer.push_str(block["text"].as_str().unwrap_or_default());
        }
        let calls: Vec<&Value> = blocks.iter().filter(|b| b["type"] == "tool_use").collect();
        if value["stop_reason"] != "tool_use" || calls.is_empty() {
            break;
        }
        answer.clear();
        messages.push(json!({"role": "assistant", "content": blocks}));
        let mut results = Vec::new();
        for call in calls {
            let name = call["name"].as_str().unwrap_or_default();
            let output = tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                output = tools.execute(name, call["input"].clone(), events, cancel) => output,
            };
            results
                .push(json!({"type": "tool_result", "tool_use_id": call["id"], "content": output}));
        }
        messages.push(json!({"role": "user", "content": results}));
    }
    if answer.trim().is_empty() {
        bail!("Anthropic không trả lời (đã hết số bước) — hãy chia nhỏ việc");
    }
    emit_text(events, "delta", answer.trim().to_owned()).await?;
    emit_text(
        events,
        "log",
        format!("Token: vào {tokens_in} (cache 0), ra {tokens_out} (suy luận 0)"),
    )
    .await?;
    for (path, added, removed) in tools.change_summaries() {
        emit_text(
            events,
            "log",
            format!("Đã sửa {path} (+{added} −{removed} dòng)"),
        )
        .await?;
    }
    events
        .send(ProviderEvent {
            name: "diff",
            data: json!({"text":tools.diff()}),
        })
        .await
        .map_err(|_| anyhow::anyhow!("Chat stream closed"))
}
