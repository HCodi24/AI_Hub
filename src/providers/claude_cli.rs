use super::{cli_support, Provider, ProviderEvent, ProviderRequest};
use crate::chat::Permission;
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct ClaudeCliProvider;

#[async_trait]
impl Provider for ClaudeCliProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if !cli_support::valid_model(&request.model) {
            bail!("Tên model chỉ được chứa chữ, số, ., _, :, / và -");
        }
        let mut args = vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("stream-json"),
            OsString::from("--verbose"),
        ];
        let permission_mode = match request.permission {
            Permission::Read => "plan",
            Permission::Write => "acceptEdits",
        };
        args.extend([
            OsString::from("--permission-mode"),
            OsString::from(permission_mode),
        ]);
        if !request.model.is_empty() {
            args.extend([OsString::from("--model"), OsString::from(&request.model)]);
        }
        if let Some(session_id) = request.session_id.as_deref() {
            if !cli_support::safe_session_id(session_id) {
                bail!("Claude session id không hợp lệ");
            }
            args.extend([OsString::from("--resume"), OsString::from(session_id)]);
        }
        let prompt = cli_support::compose_prompt(&request.messages, request.session_id.is_some());
        cli_support::run_jsonl(
            cli_support::JsonlCommand {
                cli_name: "Claude Code CLI",
                executable_name: "claude",
                missing_message: "Chưa cài Claude Code CLI: npm i -g @anthropic-ai/claude-code",
                configured_executable: request.provider.executable.clone(),
                folder: &request.folder,
                args,
                prompt,
            },
            events,
            cancel,
            parse_claude_line,
        )
        .await
    }
}

fn parse_claude_line(line: &str) -> Vec<ProviderEvent> {
    if line.trim().is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return vec![ProviderEvent::text(
            "log",
            format!("Claude output: {}", cli_support::one_line(line, 240)),
        )];
    };
    let event_type = value["type"].as_str().unwrap_or("unknown");
    let mut events = Vec::new();
    if let Some(session_id) = value["session_id"]
        .as_str()
        .filter(|id| cli_support::safe_session_id(id))
    {
        events.push(ProviderEvent {
            name: "session",
            data: serde_json::json!({ "session_id": session_id }),
        });
    }
    match event_type {
        "system" | "user" => {}
        "assistant" => {
            if let Some(blocks) = value["message"]["content"].as_array() {
                for block in blocks {
                    match block["type"].as_str().unwrap_or("") {
                        "text" => {
                            if let Some(text) =
                                block["text"].as_str().filter(|text| !text.is_empty())
                            {
                                events.push(ProviderEvent::text("delta", text.to_owned()));
                            }
                        }
                        "tool_use" => {
                            events.push(ProviderEvent::text("log", summarize_tool_use(block)))
                        }
                        _ => {}
                    }
                }
            }
        }
        "result" => {
            if value["is_error"].as_bool() == Some(true) {
                let message = value["result"]
                    .as_str()
                    .or_else(|| value["error"].as_str())
                    .unwrap_or("Claude Code CLI trả về lỗi");
                events.push(ProviderEvent::text("error", message.to_owned()));
            }
            if let Some(usage) =
                cli_support::token_usage_log(&value["usage"], value.get("total_cost_usd"))
            {
                events.push(ProviderEvent::text("log", usage));
            }
        }
        _ => events.push(ProviderEvent::text(
            "log",
            format!(
                "Claude event {event_type}: {}",
                cli_support::one_line(&value.to_string(), 220)
            ),
        )),
    }
    events
}

fn summarize_tool_use(block: &Value) -> String {
    let name = block["name"].as_str().unwrap_or("tool");
    let input = &block["input"];
    match name {
        "Read" => {
            let path = input["file_path"].as_str().unwrap_or("file");
            format!("Read file {}", cli_support::one_line(path, 220))
        }
        "Bash" => {
            let command = input["command"].as_str().unwrap_or("command");
            format!("Bash: {}", cli_support::one_line(command, 220))
        }
        _ => format!(
            "{}: {}",
            cli_support::one_line(name, 80),
            cli_support::one_line(&input.to_string(), 220)
        ),
    }
}
