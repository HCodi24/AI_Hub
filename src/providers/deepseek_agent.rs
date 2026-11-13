use super::agent::AgentTools;
use super::{emit_text, Provider, ProviderEvent, ProviderRequest};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const SYSTEM_WRITE: &str = concat!(
    "You are a coding assistant with file tools. Work only in the supplied folder with the tools. ",
    "Do not run commands. Answer the user's request; make only the changes they asked for, ",
    "then give a concise report."
);
const SYSTEM_FULL: &str = concat!(
    "You are a coding assistant with file tools. Work only in the supplied folder with the tools. ",
    "You can run commands in the working folder with run_command; prefer small, safe commands; ",
    "never run destructive commands the user did not ask for. Answer the user's request; make only the changes they asked for, ",
    "then give a concise report."
);
const SYSTEM_MCP: &str = concat!(
    "You are an assistant with tools provided by MCP servers. Use them when they help; ",
    "you cannot read or edit files or run commands except through those tools. Answer concisely."
);
const SYSTEM_READ: &str = concat!(
    "You are a coding assistant with READ-ONLY file tools (list and read). ",
    "Work only in the supplied folder. You cannot edit files or run commands. ",
    "Explore the folder as needed and answer the user's question concisely."
);

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
        let system = if !files {
            SYSTEM_MCP
        } else {
            match request.permission {
                super::Permission::Read => SYSTEM_READ,
                super::Permission::Write => SYSTEM_WRITE,
                super::Permission::Full => SYSTEM_FULL,
            }
        };
        let mut system = system.to_string();
        if files {
            system.push_str(&format!("\n\n{}", crate::core::rule::deepseek::RULE));
            if super::is_deepseek_flash(&request.model) {
                system.push_str(&format!("\n\n{}", crate::core::rule::deepseek_flash::RULE));
            }
        }
        let mut messages = vec![json!({"role":"system","content":system})];
        if excerpts.is_empty() {
            let last = request.messages.len().saturating_sub(1);
            for (index, m) in request.messages.iter().enumerate() {
                // Only the newest message carries its images (if the model has no vision the API reports it).
                messages.push(if index == last {
                    super::openai_compat::openai_message(m)?
                } else {
                    json!({"role":m.role,"content":m.content})
                });
            }
        } else {
            let history = request
                .messages
                .iter()
                .filter(|m| m.role != "user" || m.content != task)
                .map(|m| json!({"role":m.role,"content":m.content}));
            messages.extend(history);
            let mut prompt = format!(
                "Working directory: {}\n\nTask:\n{}\n\nPreloaded file excerpts (FILE blocks):\n",
                root.display(),
                task
            );
            for (path, lines) in excerpts {
                prompt.push_str(&format!("\n--- {path} ---\n{lines}\n"));
            }
            messages.push(json!({"role":"user","content":prompt}));
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
        let mut tokens_in = 0u64;
        let mut tokens_out = 0u64;
        let mut tokens_cached = 0u64;
        let mut tokens_reasoning = 0u64;
        let mut final_text = String::new();
        let mut failure: Option<anyhow::Error> = None;
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
            let mut body = json!({"model":request.model,"messages":messages,"tools":tools,"max_tokens":8192,"temperature":0.2,"stream":false});
            // Last allowed step: forbid more tool calls so the model must report what it did instead of ending silently.
            if step + 1 == max_steps {
                body["tool_choice"] = json!("none");
                // Without this note the model still tries to call tools and prints its raw DSML tool markup as the answer.
                let mut last = messages.clone();
                last.push(json!({"role":"user","content":"Tool budget is used up: do not call any more tools. Report what you found so far, what is still unknown, and which files to look at next."}));
                body["messages"] = json!(last);
            }
            if super::is_deepseek_flash(&request.model) {
                body["reasoning_effort"] = json!(super::api_reasoning_effort(&request.model, request.effort));
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
            tokens_in =
                tokens_in.saturating_add(value["usage"]["prompt_tokens"].as_u64().unwrap_or(0));
            tokens_out = tokens_out
                .saturating_add(value["usage"]["completion_tokens"].as_u64().unwrap_or(0));
            // DeepSeek reports `prompt_cache_hit_tokens`; OpenAI-style hosts use `prompt_tokens_details.cached_tokens`.
            let usage = &value["usage"];
            tokens_cached = tokens_cached.saturating_add(
                usage["prompt_cache_hit_tokens"].as_u64()
                    .or(usage["prompt_tokens_details"]["cached_tokens"].as_u64())
                    .unwrap_or(0),
            );
            tokens_reasoning = tokens_reasoning
                .saturating_add(usage["completion_tokens_details"]["reasoning_tokens"].as_u64().unwrap_or(0));
            let msg = &value["choices"][0]["message"];
            if msg.is_null() {
                failure = Some(anyhow::anyhow!(
                    "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                ));
                break;
            }
            let finish_reason = value["choices"][0]["finish_reason"].as_str().unwrap_or("");
            let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
            let content = msg["content"].as_str().unwrap_or_default();
            if calls.is_empty() {
                // Drop leaked tool-call markup (`<｜DSML｜function_calls>...`); it is not an answer.
                let visible = content.find("DSML").map_or(content, |at| content[..at].trim_end_matches(|c: char| c == '<' || c.is_whitespace()));
                final_text = visible.trim().to_owned();
                // A plain text answer with no tool call is valid (a question, an explanation): only an empty one is a failure.
                if final_text.is_empty() {
                    failure = Some(anyhow::anyhow!(
                        "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                    ));
                }
                break;
            }
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
        if !final_text.is_empty() {
            emit_text(&events, "delta", final_text).await?;
        }
        if tokens_in + tokens_out > 0 {
            emit_text(
                &events,
                "log",
                format!("Token: vào {tokens_in} (cache {tokens_cached}), ra {tokens_out} (suy luận {tokens_reasoning})"),
            )
            .await?;
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

pub fn parse_edit_only(input: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        let heading = line.trim().trim_matches('`').trim();
        if let Some(header) = heading.strip_prefix("FILE: ") {
            let path = header
                .split_once(" LINES ")
                .map(|(path, lines)| format!("{path} (lines {lines})"))
                .unwrap_or_else(|| header.to_owned());
            let mut content = String::new();
            for next in lines.by_ref() {
                if next.trim().starts_with("```") {
                    break;
                }
                content.push_str(next);
                content.push('\n');
            }
            out.push((path.trim().to_owned(), content.trim_end().to_owned()));
        }
    }
    out
}
