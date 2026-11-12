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
        if request
            .messages
            .iter()
            .any(|message| !message.attachments.is_empty())
        {
            bail!("DeepSeek làm việc chưa hỗ trợ ảnh — hãy dùng nguồn khác");
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
        let edits_requested = files && can_write && task_asks_edits(task);
        let system = if !files {
            SYSTEM_MCP
        } else {
            match request.permission {
                super::Permission::Read => SYSTEM_READ,
                super::Permission::Write => SYSTEM_WRITE,
                super::Permission::Full => SYSTEM_FULL,
            }
        };
        let mut messages = vec![json!({"role":"system","content":system})];
        if excerpts.is_empty() {
            messages.extend(
                request
                    .messages
                    .iter()
                    .map(|m| json!({"role":m.role,"content":m.content})),
            );
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
        let mcp =
            super::mcp::McpTools::start(&request.mcp, &request.folder, can_write, &events).await;
        let mut agent_tools = AgentTools::new(
            root,
            request.permission,
            request.mutis.deepseek_max_file_lines,
            files,
            mcp,
        );
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
        let mut had_tool = false;
        let mut final_text = String::new();
        let mut failure: Option<anyhow::Error> = None;
        for step in 0..request.mutis.deepseek_max_steps.clamp(5, 40) {
            if cancel.is_cancelled() {
                failure = Some(anyhow::anyhow!("Đã dừng"));
                break;
            }
            steps = step + 1;
            let body = json!({"model":request.model,"messages":messages,"tools":tools,"max_tokens":8192,"temperature":0.2,"stream":false});
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
            let msg = &value["choices"][0]["message"];
            if msg.is_null() {
                failure = Some(anyhow::anyhow!(
                    "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                ));
                break;
            }
            let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
            let content = msg["content"].as_str().unwrap_or_default();
            if calls.is_empty() {
                final_text = content.trim().to_owned();
                if final_text.is_empty() || (edits_requested && !had_tool) {
                    failure = Some(anyhow::anyhow!(
                        "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                    ));
                }
                break;
            }
            had_tool = true;
            messages.push(json!({"role":"assistant","content":if content.is_empty(){Value::Null}else{json!(content)},"tool_calls":calls}));
            for call in calls {
                if cancel.is_cancelled() {
                    failure = Some(anyhow::anyhow!("Đã dừng"));
                    break;
                }
                let id = call["id"].as_str().unwrap_or_default();
                let name = call["function"]["name"].as_str().unwrap_or_default();
                let args = serde_json::from_str::<Value>(
                    call["function"]["arguments"].as_str().unwrap_or("{}"),
                )
                .unwrap_or(json!({}));
                let result = tokio::select! {
                    _ = cancel.cancelled() => {
                        failure = Some(anyhow::anyhow!("Đã dừng"));
                        break;
                    }
                    result = agent_tools.execute(name, args, &events, &cancel) => result,
                };
                messages.push(json!({"role":"tool","tool_call_id":id,"content":result}));
            }
            if failure.is_some() {
                break;
            }
        }
        if failure.is_none() && final_text.is_empty() {
            failure = Some(anyhow::anyhow!(
                "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
            ));
        }
        if !final_text.is_empty() {
            emit_text(&events, "delta", final_text).await?;
        }
        if tokens_in + tokens_out > 0 {
            emit_text(
                &events,
                "log",
                format!("Token: vào {tokens_in} (cache 0), ra {tokens_out} (suy luận 0)"),
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
fn task_asks_edits(task: &str) -> bool {
    let text = task.to_lowercase();
    [
        "edit",
        "modify",
        "change",
        "write",
        "create",
        "fix",
        "implement",
        "sửa",
        "chỉnh",
        "ghi",
        "tạo",
        "thêm",
        "xóa",
        "xoá",
        "cập nhật",
    ]
    .iter()
    .any(|word| text.contains(word))
}
