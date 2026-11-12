use super::{environment, transport::Client};
use crate::{
    chat::{Effort, Permission},
    providers::{cli_support, Provider, ProviderEvent, ProviderRequest},
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::{collections::HashSet, path::Path, time::Duration};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct CodexProvider;

#[async_trait]
impl Provider for CodexProvider {
    async fn run(
        &self,
        mut request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let config_path = crate::config::config_file_path()?;
        let settings = environment::settings()?;
        let home = environment::home(&config_path, &request.provider)?;
        let executable =
            cli_support::resolve_executable("codex", Some(&request.provider.executable))
                .context("Chưa tìm thấy Codex; vào Cài đặt → Nguồn AI để kết nối")?;
        let folder = Path::new(&request.folder);
        let prefix = environment::session_prefix(&settings, &request.provider, &request.folder);
        let native_id = request
            .session_id
            .as_deref()
            .and_then(|id| id.strip_prefix(&prefix))
            .map(str::to_owned);
        let mut client = Client::start(&executable, &home, folder).await?;
        let result = async {
            let account = client.call("account/read", json!({"refreshToken":false})).await?;
            if account["account"].is_null() { bail!("Codex của MutiS chưa đăng nhập; dùng Tìm Codex CLI hoặc Đăng nhập qua web trong Cài đặt"); }
            let sandbox = if request.permission == Permission::Read { "read-only" } else { "workspace-write" };
            let context = environment::project_context(folder, &settings)?;
            let mcp = request.mcp.iter().filter(|server| server.enabled).map(|server| (
                server.name.clone(), json!({"command":server.command,"args":server.args,"env":server.env})
            )).collect::<serde_json::Map<String, serde_json::Value>>();
            let params = json!({"cwd":request.folder,"model":request.model,"approvalPolicy":"never","sandbox":sandbox,
                "developerInstructions":context,"selectedCapabilityRoots":[],"config":{"mcp_servers":mcp}});
            let mut resumed = native_id.is_some();
            let thread = if let Some(id) = native_id {
                let mut resume = params.clone();
                resume["threadId"] = json!(id);
                match client.call("thread/resume", resume).await {
                    Ok(value) => value,
                    Err(_) => {
                        resumed = false;
                        // Rebuild app context if the persisted native thread is no longer available.
                        if request.messages.last().is_some_and(|m| m.content.contains(crate::features::memory::INSTRUCTION)) {
                            let config: crate::config::Config = serde_json::from_slice(&std::fs::read(&config_path)?)?;
                            request.session_id = None;
                            let memory = crate::features::memory::memory_context(&config_path, &request.folder, &config.memory);
                            if !memory.is_empty() {
                                if let Some(message) = request.messages.last_mut() { message.content = format!("{memory}\n\n{}", message.content); }
                            }
                        }
                        crate::providers::emit_text(&events, "log", "Đang khôi phục phiên Codex từ ngữ cảnh MutiS".into()).await?;
                        client.call("thread/start", params).await?
                    }
                }
            } else { client.call("thread/start", params).await? };
            let id = thread["thread"]["id"].as_str().context("Codex không trả mã phiên")?.to_owned();
            events.send(ProviderEvent { name:"session", data:json!({"session_id":format!("{prefix}{id}")}) }).await?;
            let mut input = vec![json!({"type":"text","text":cli_support::compose_prompt(&request.messages, resumed)})];
            if let Some(dir) = crate::providers::latest_chat_attachment_dir(&request.messages) {
                for image in crate::providers::latest_image_paths(&request.messages, &dir)? {
                    input.push(json!({"type":"localImage","path":image}));
                }
            }
            let effort = match request.effort { Effort::Default => None, Effort::Low => Some("low"), Effort::Medium => Some("medium"), Effort::High => Some("high"), Effort::Xhigh => Some("xhigh"), Effort::Max => Some("max") };
            let mut turn = json!({"threadId":id,"input":input});
            if let Some(effort) = effort { turn["effort"] = json!(effort); }
            client.call("turn/start", turn).await?;
            let mut seen = HashSet::new();
            let mut last_message = String::new();
            let mut final_reply = String::new();
            let mut resumes = 0;
            loop {
                let limit = if request.mutis.effective(request.mutis.codex_watchdog) {
                    Duration::from_secs(u64::from(request.mutis.watchdog_minutes) * 60)
                } else { Duration::from_secs(7200) };
                let value = tokio::select! {
                    _ = cancel.cancelled() => { return Ok(()); },
                    value = tokio::time::timeout(limit, client.next()) => value.context("Codex không phản hồi; đã dừng lượt chạy")??,
                };
                if value.get("id").is_some() && value.get("method").is_some() {
                    client.reject_request(&value).await?;
                    continue;
                }
                let params = &value["params"];
                match value["method"].as_str().unwrap_or("") {
                    "item/agentMessage/delta" => {
                        let item = params["itemId"].as_str().unwrap_or("");
                        if !last_message.is_empty() && last_message != item { crate::providers::emit_text(&events,"delta","\n\n".into()).await?; }
                        last_message = item.into(); seen.insert(item.to_owned());
                        let text = params["delta"].as_str().unwrap_or("");
                        final_reply.push_str(text);
                        crate::providers::emit_text(&events,"delta",text.into()).await?;
                    }
                    "item/reasoning/summaryTextDelta" | "item/reasoning/textDelta" => {
                        crate::providers::emit_text(&events,"reasoning",params["delta"].as_str().unwrap_or("").into()).await?;
                    }
                    "item/completed" => {
                        let item = &params["item"];
                        if item["type"] == "agentMessage" && !seen.contains(item["id"].as_str().unwrap_or("")) {
                            let text = item["text"].as_str().unwrap_or("");
                            if !final_reply.is_empty() { crate::providers::emit_text(&events,"delta","\n\n".into()).await?; }
                            final_reply.push_str(text);
                            crate::providers::emit_text(&events,"delta",text.into()).await?;
                        } else if item["type"] == "commandExecution" {
                            let command = item["command"].as_str().unwrap_or("");
                            crate::providers::emit_text(&events,"log",format!("Command: {}",crate::core::text::truncate_chars(command,220))).await?;
                        }
                    }
                    "turn/diff/updated" => {
                        crate::providers::emit_text(&events,"diff",params["diff"].as_str().unwrap_or("").into()).await?;
                    }
                    "thread/tokenUsage/updated" => {
                        let usage = &params["tokenUsage"]["last"];
                        crate::providers::emit_text(&events,"log",format!("Token: vào {}, ra {}",usage["inputTokens"],usage["outputTokens"])).await?;
                    }
                    "turn/completed" => {
                        if params["turn"]["status"] == "failed" { bail!("Codex không hoàn thành được yêu cầu; kiểm tra tài khoản, model và kết nối"); }
                        let asking = final_reply.contains("```mutis-questions");
                        if !asking && request.mutis.effective(request.mutis.codex_auto_resume) && resumes < request.mutis.resume_max.min(3)
                            && crate::providers::codex_cli::indicates_early_stop(&final_reply) {
                            resumes += 1; final_reply.clear();
                            client.call("turn/start",json!({"threadId":id,"input":[{"type":"text","text":"Continue the unfinished work within the original scope."}]})).await?;
                        } else { break; }
                    }
                    "error" if params["willRetry"] != true => bail!("Codex báo lỗi; kiểm tra tài khoản, model và kết nối"),
                    _ => {}
                }
            }
            Ok(())
        }.await;
        client.close().await;
        result
    }
}
