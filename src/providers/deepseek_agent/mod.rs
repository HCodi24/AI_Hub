use super::agent::AgentTools;
use super::{emit_text, Provider, ProviderEvent, ProviderRequest};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod history;
mod prompt;
#[cfg(test)]
mod tests;
mod usage;

use history::{chats_root, drop_unanswered_calls, trim_history};
use usage::Usage;
pub use prompt::parse_edit_only;
use prompt::{build_messages, strip_dsml, system_prompt};

#[async_trait]
impl Provider for DeepSeekAgentProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        // Only the message being answered matters: images in earlier turns are simply not sent (this loop sends
        // text only), so one old screenshot must not make every later question fail.
        let has_older_images = request
            .messages
            .iter()
            .rev()
            .skip(1)
            .any(|message| !message.attachments.is_empty());
        if has_older_images {
            emit_text(&events, "log", "Chỉ gửi ảnh của tin nhắn mới nhất; ảnh ở các tin trước không gửi lại".to_owned()).await?;
        }
        if request.provider.api_key.trim().is_empty() {
            bail!("API key chưa được cấu hình cho nhà cung cấp này");
        }
        let base = request.provider.base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            bail!("URL cơ sở chưa được cấu hình cho nhà cung cấp này");
        }
        let root = PathBuf::from(&request.folder)
            .canonicalize()
            .context("Thư mục làm việc không tồn tại")?;
        if !root.is_dir() {
            bail!("Thư mục làm việc không hợp lệ");
        }
        let task = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or_default();
        // Full grants file tools and run_command; Read and Write retain their existing scope.
        let can_write = matches!(
            request.permission,
            super::Permission::Write | super::Permission::Full
        );
        let excerpts = parse_edit_only(task);
        // "mcp_agent" = the same loop with MCP tools only (no file tools).
        let files = request.provider.kind != "mcp_agent";
        let system = system_prompt(&request, files);
        let mut messages = build_messages(&request, &root, task, excerpts, system)?;
        let session_key = request.session_id.clone().filter(|id| id.starts_with(super::worker_session::PREFIX));
        let root_dir = chats_root();
        if let (Some(key), Some(dir)) = (&session_key, &root_dir) {
            let prior = super::worker_session::load(dir, key);
            if !prior.is_empty() {
                let mut newest = messages.pop().unwrap_or_default();
                if let Some(text) = newest["content"].as_str() {
                    newest["content"] = json!(format!("{text}\nThis is the same session: the conversation above is your earlier work in this chat. Files may have changed since you read them (by you, another task or the coordinator): re-read a file only when you need its exact current text."));
                }
                emit_text(&events, "log", format!("Giữ phiên DeepSeek: {} tin cũ", prior.len())).await?;
                messages.extend(prior);
                messages.push(newest);
            }
        }
        // Only the explicit "mcp_agent" source may use MCP; the normal DeepSeek agent never does.
        let mcp_servers = if files { &[][..] } else { &request.mcp[..] };
        let mcp =
            super::mcp::McpTools::start(mcp_servers, &request.folder, can_write, &events).await;
        let mut agent_tools = AgentTools::new(
            root,
            request.permission,
            request.mutis.deepseek_max_file_lines,
            files,
            mcp,
        )
        .with_scope(request.write_scope.clone());
        let tools = agent_tools.openai_definitions();
        if tools.is_empty() {
            bail!("Không có công cụ nào dùng được (kiểm tra MCP trong Cài đặt)");
        }
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let mut steps = 0;
        let mut usage = Usage::default();
        let mut final_text = String::new();
        let mut failure: Option<anyhow::Error> = None;
        // Set once a reply was cut off while thinking: later steps think at low effort instead of failing the task.
        let mut cut_off = false;
        let max_steps = request.mutis.deepseek_max_steps.clamp(5, 40);
        for step in 0..max_steps {
            if cancel.is_cancelled() {
                failure = Some(anyhow::anyhow!("Đã dừng"));
                break;
            }
            steps = step + 1;
            // Exploring forever is the usual way a worker ends without editing: warn while there is still budget to act.
            if can_write && files && max_steps >= 8 && step + 4 == max_steps && agent_tools.change_summaries().is_empty() {
                messages.push(json!({"role":"user","content":"Only 4 steps are left and you have not changed any file. Stop exploring now: make the edit with edit_file or write_file, or report exactly what blocks you."}));
            }
            let mut body = json!({"model":request.model,"messages":messages,"tools":tools,"temperature":0.2,"stream":false});
            // Last allowed step: forbid more tool calls so the model must report what it did instead of ending silently.
            if step + 1 == max_steps {
                body["tool_choice"] = json!("none");
                // Without this note the model still tries to call tools and prints its raw DSML tool markup as the answer.
                let mut last = messages.clone();
                last.push(json!({"role":"user","content":"Tool budget is used up: do not call any more tools. Report what you found so far, what is still unknown, and which files to look at next."}));
                body["messages"] = json!(last);
            }
            if super::is_deepseek_flash(&request.model) {
                body["reasoning_effort"] = json!(if cut_off { Some("low") } else { super::api_reasoning_effort(&request.model, request.effort) });
            }
            let response = super::send_with_retry(
                &client,
                &format!("{base}/chat/completions"),
                vec![(
                    "Authorization".into(),
                    format!("Bearer {}", request.provider.api_key),
                )],
                &body,
                &request.mutis,
                &events,
                &cancel,
            )
            .await;
            let response = match response {
                Ok(v) => v,
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            };
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                failure = Some(anyhow::anyhow!(
                    "DeepSeek API HTTP {status}: {}",
                    super::safe_error_body(&body, &request.provider.api_key)
                ));
                break;
            }
            let decoded = tokio::select! {
                _ = cancel.cancelled() => { failure = Some(anyhow::anyhow!("Đã dừng")); break; }
                result = response.json::<Value>() => result,
            };
            let value: Value = match decoded {
                Ok(v) => v,
                Err(e) => {
                    failure = Some(e.into());
                    break;
                }
            };
            usage.add(&value["usage"]);
            let msg = &value["choices"][0]["message"];
            let finish_reason = value["choices"][0]["finish_reason"].as_str().unwrap_or("");
            // Reasoning tokens count toward the host's output cap: a long think can still end the reply with no content and no tool call.
            let empty_reply = msg.is_null() || (msg["tool_calls"].as_array().map_or(true, Vec::is_empty) && strip_dsml(msg["content"].as_str().unwrap_or_default()).is_empty());
            if empty_reply && finish_reason == "length" && !cut_off {
                cut_off = true;
                messages.push(json!({"role":"user","content":"Your last reply was cut off while thinking. Think briefly, then make the next tool call or answer."}));
                continue;
            }
            if msg.is_null() {
                failure = Some(anyhow::anyhow!(
                    "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                ));
                break;
            }
            let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
            let content = msg["content"].as_str().unwrap_or_default();
            if calls.is_empty() {
                // Drop leaked tool-call markup (`<｜DSML｜function_calls>...`); it is not an answer.
                final_text = strip_dsml(content).to_owned();
                // A plain text answer with no tool call is valid (a question, an explanation): only an empty one is a failure.
                if final_text.is_empty() {
                    failure = Some(anyhow::anyhow!(
                        "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                    ));
                }
                break;
            }
            emit_text(&events, "log", format!("Bước {steps}/{max_steps}: {} lệnh", calls.len())).await?;
            let mut assistant = json!({"role":"assistant","content":if content.is_empty(){Value::Null}else{json!(content)},"tool_calls":calls});
            // Thinking tool calls must retain reasoning for the next API request.
            if let Some(reasoning) = msg.get("reasoning_content") {
                assistant["reasoning_content"] = reasoning.clone();
            }
            messages.push(assistant);
            for call in calls {
                if cancel.is_cancelled() {
                    failure = Some(anyhow::anyhow!("Đã dừng"));
                    break;
                }
                let id = call["id"].as_str().unwrap_or_default();
                let name = call["function"]["name"].as_str().unwrap_or_default();
                // Invalid arguments used to become `{}` (path ".", no content): the call then failed for a reason the model could not see.
                let result = match serde_json::from_str::<Value>(call["function"]["arguments"].as_str().unwrap_or("{}")) {
                    Ok(args) => tokio::select! {
                        _ = cancel.cancelled() => {
                            failure = Some(anyhow::anyhow!("Đã dừng"));
                            break;
                        }
                        result = agent_tools.execute(name, args, &events, &cancel) => result,
                    },
                    Err(error) => {
                        let _ = emit_text(&events, "log", format!("Lỗi: tham số của {name} không phải JSON hợp lệ")).await;
                        let cause = if finish_reason == "length" { "your reply was cut off at the output limit".to_owned() } else { error.to_string() };
                        format!("error: the arguments of {name} are not valid JSON ({cause}). Nothing was executed. Resend the call with valid JSON. To create a large file, write_file only its first part (about 120 lines at most), then add the rest with edit_file.")
                    }
                };
                messages.push(json!({"role":"tool","tool_call_id":id,"content":result}));
            }
            if failure.is_some() {
                break;
            }
        }
        if failure.is_none() && final_text.is_empty() {
            failure = Some(anyhow::anyhow!(
                "DeepSeek dùng hết {steps} bước công cụ mà chưa kết luận — việc quá rộng hoặc nó tìm file chưa ra; nói rõ file/đường dẫn cần sửa hoặc tăng số bước trong Cài đặt MutiS"
            ));
        }
        if let (Some(key), Some(dir)) = (session_key, root_dir) {
            let mut history = messages[1..].to_vec();
            if !final_text.is_empty() {
                history.push(json!({"role":"assistant","content":final_text}));
            }
            drop_unanswered_calls(&mut history);
            super::worker_session::save(&dir, &key, &trim_history(&history));
        }
        if !final_text.is_empty() {
            emit_text(&events, "delta", final_text).await?;
        }
        if let Some(line) = usage.line() {
            emit_text(&events, "log", line).await?;
        }
        let diff = agent_tools.diff();
        for (path, added, removed) in agent_tools.change_summaries() {
            emit_text(
                &events,
                "log",
                format!("Đã sửa {path} (+{added} −{removed} dòng)"),
            )
            .await?;
        }
        events
            .send(ProviderEvent {
                name: "diff",
                data: json!({"text":diff}),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Chat stream closed"))?;
        if let Some(error) = failure {
            return Err(error);
        }
        let _ = steps;
        Ok(())
    }
}

pub struct DeepSeekAgentProvider;
