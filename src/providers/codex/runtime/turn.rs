use super::approval;
use crate::{
    chat::{Effort, Permission},
    providers::{cli_support, ProviderEvent, ProviderRequest, codex::{compact, gate::Gate, transport::Client}},
};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{collections::HashSet, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
struct TurnState {
    seen: HashSet<String>,
    last_message: String,
    final_reply: String,
    resumes: u32,
    denial_sent: bool,
}

pub(super) fn effort_arg(effort: Effort) -> Option<&'static str> {
    match effort {
        Effort::Default => None,
        Effort::None => Some("none"),
        Effort::Minimal => Some("minimal"),
        Effort::Low => Some("low"),
        Effort::Medium => Some("medium"),
        Effort::High => Some("high"),
        Effort::Xhigh => Some("xhigh"),
        Effort::Max => Some("max"),
        Effort::Ultra => Some("ultra"),
    }
}

pub(super) async fn run(
    client: &mut Client,
    request: &ProviderRequest,
    gate: &Gate,
    id: &str,
    resumed: bool,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> Result<()> {
    let mut input = vec![json!({"type":"text","text":cli_support::compose_prompt_for(&request.messages, resumed, "codex_cli")})];
    if let Some(dir) = crate::providers::latest_chat_attachment_dir(&request.messages) {
        for image in crate::providers::latest_image_paths(&request.messages, &dir)? {
            input.push(json!({"type":"localImage","path":image}));
        }
    }
    let effort = effort_arg(request.effort);
    let mut turn = json!({"threadId":id,"input":input});
    if let Some(effort) = effort { turn["effort"] = json!(effort); }
    client.call("turn/start", turn).await?;
    let mut state = TurnState::default();
    loop {
        let limit = if request.mutis.effective(request.mutis.codex_watchdog) {
            Duration::from_secs(u64::from(request.mutis.watchdog_minutes) * 60)
        } else { Duration::from_secs(7200) };
        let value = tokio::select! {
            _ = cancel.cancelled() => { return Ok(()); },
            value = tokio::time::timeout(limit, client.next()) => value.context("Codex không phản hồi; đã dừng lượt chạy")??,
        };
        if value.get("id").is_some() && value.get("method").is_some() {
            if !approval::handle(client, gate, request.permission, &value, events, cancel, &mut state.denial_sent).await? {
                return Ok(());
            }
            continue;
        }
        let params = &value["params"];
        match value["method"].as_str().unwrap_or("") {
            "item/agentMessage/delta" => {
                let item = params["itemId"].as_str().unwrap_or("");
                if !state.last_message.is_empty() && state.last_message != item { crate::providers::emit_text(events,"delta","\n\n".into()).await?; }
                state.last_message = item.into(); state.seen.insert(item.to_owned());
                let text = params["delta"].as_str().unwrap_or("");
                state.final_reply.push_str(text);
                crate::providers::emit_text(events,"delta",text.into()).await?;
            }
            "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                crate::providers::emit_text(events,"reasoning",params["delta"].as_str().unwrap_or("").into()).await?;
            }
            "item/completed" => {
                let item = &params["item"];
                if item["type"] == "agentMessage" && !state.seen.contains(item["id"].as_str().unwrap_or("")) {
                    let text = item["text"].as_str().unwrap_or("");
                    if !state.final_reply.is_empty() { crate::providers::emit_text(events,"delta","\n\n".into()).await?; }
                    state.final_reply.push_str(text);
                    crate::providers::emit_text(events,"delta",text.into()).await?;
                } else if item["type"] == "commandExecution" {
                    let command = item["command"].as_str().unwrap_or("");
                    crate::providers::emit_text(events,"log",format!("Command: {}",crate::core::text::truncate_chars(command,220))).await?;
                    // shortcut: a failed command in the read-only sandbox is taken as a permission block (the app-server gives no reason); other causes show the same card.
                    if request.permission == Permission::Read && item["status"] == "failed" && !state.denial_sent {
                        state.denial_sent = true;
                        approval::permission_denied(events).await;
                    }
                }
            }
            "turn/diff/updated" => {
                crate::providers::emit_text(events,"diff",params["diff"].as_str().unwrap_or("").into()).await?;
            }
            "thread/tokenUsage/updated" => {
                let usage = &params["tokenUsage"]["last"];
                crate::providers::emit_text(events,"log",format!("Token: vào {} (cache {}), ra {}",usage["inputTokens"],usage["cachedInputTokens"].as_u64().unwrap_or(0),usage["outputTokens"])).await?;
                // The last model call read the whole thread, so its input is the thread's current size.
                if let Some(tokens) = usage["inputTokens"].as_u64() {
                    compact::note_size(id, tokens);
                    crate::providers::emit_text(events,"thread_tokens",tokens.to_string()).await?;
                }
            }
            "turn/completed" => {
                if params["turn"]["status"] == "failed" { bail!("Codex không hoàn thành được yêu cầu; kiểm tra tài khoản, model và kết nối"); }
                let asking = state.final_reply.contains("```mutis-questions");
                if !asking && request.mutis.effective(request.mutis.codex_auto_resume) && state.resumes < request.mutis.resume_max.min(3)
                    && crate::providers::codex_cli::indicates_early_stop(&state.final_reply) {
                    state.resumes += 1; state.final_reply.clear();
                    client.call("turn/start",json!({"threadId":id,"input":[{"type":"text","text":"Continue the unfinished work within the original scope."}]})).await?;
                } else { break; }
            }
            "error" if params["willRetry"] != true => {
                let detail = params["error"]["message"].as_str().or(params["message"].as_str()).unwrap_or("");
                if detail.contains("not supported when using Codex with a ChatGPT account") {
                    bail!("Gói ChatGPT hiện tại (ví dụ Free) không dùng được model «{}». Chọn model khác ở thanh chọn model, hoặc đổi model của vai Codex trong Cài đặt › MutiS, hoặc nâng gói.", request.model);
                }
                bail!("Codex báo lỗi: {}", crate::core::text::truncate_chars(detail, 400))
            }
            _ => {}
        }
    }
    Ok(())
}
