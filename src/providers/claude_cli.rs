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
        mut request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if !cli_support::valid_model(&request.model) {
            bail!("Tên model chỉ được chứa chữ, số, ., _, :, / và -");
        }
        let config_path = crate::config::config_file_path()?;
        let settings = super::claude::environment::settings()?;
        let home = super::claude::environment::home(&config_path, &request.provider)?;
        let prefix = super::claude::environment::session_prefix(&settings, &request.provider, &request.folder);
        let native = request.session_id.as_deref().and_then(|id| id.strip_prefix(&prefix)).map(str::to_owned);
        if request.session_id.is_some() && native.is_none() {
            let config: crate::config::Config = serde_json::from_slice(&std::fs::read(&config_path)?)?;
            if request.messages.last().is_some_and(|m| m.content.contains(crate::features::memory::INSTRUCTION)) {
                let memory = crate::features::memory::memory_context(&config_path, &request.folder, &config.memory);
                if let Some(message) = request.messages.last_mut() { message.content = format!("{memory}\n\n{}", message.content); }
            }
        }
        request.session_id = native;
        let mut args = vec![
            OsString::from("-p"),
            OsString::from("--output-format"),
            OsString::from("stream-json"),
            OsString::from("--verbose"),
            OsString::from("--include-partial-messages"),
        ];
        args.extend(super::claude::environment::launch_args(&request.mcp));
        let context = super::claude::sources::project_context(std::path::Path::new(&request.folder), &settings)?;
        if !context.is_empty() { args.extend([OsString::from("--append-system-prompt"), context.into()]); }
        args.extend(permission_args(request.permission));
        if !request.model.is_empty() {
            args.extend([OsString::from("--model"), OsString::from(&request.model)]);
        }
        let effort = match request.effort {
            super::Effort::Default => None,
            super::Effort::Low => Some("low"),
            super::Effort::Medium => Some("medium"),
            super::Effort::High => Some("high"),
            super::Effort::Xhigh => Some("xhigh"),
            super::Effort::Max => Some("max"),
        };
        if let Some(effort) = effort {
            args.extend([OsString::from("--effort"), OsString::from(effort)]);
        }
        if let Some(session_id) = request.session_id.as_deref() {
            if !cli_support::safe_session_id(session_id) {
                bail!("Claude session id không hợp lệ");
            }
            args.extend([OsString::from("--resume"), OsString::from(session_id)]);
        }
        let mut prompt =
            cli_support::compose_prompt_for(&request.messages, request.session_id.is_some(), "claude_cli");
        if let Some(directory) = super::latest_chat_attachment_dir(&request.messages) {
            let paths = super::latest_image_paths(&request.messages, &directory)?;
            args.extend([
                OsString::from("--add-dir"),
                directory.as_os_str().to_owned(),
            ]);
            if !paths.is_empty() {
                prompt.push_str("\n\n");
                for path in paths {
                    prompt.push_str(&format!("Ảnh đính kèm: {}\n", path.display()));
                }
                prompt.push_str("Hãy dùng công cụ đọc tệp để xem ảnh trên.");
            }
        }
        let parser = std::sync::Mutex::new(ClaudeParser {
            permission: request.permission,
            ..Default::default()
        });
        let result = cli_support::run_jsonl(
            cli_support::JsonlCommand {
                config_dir: Some(home),
                cli_name: "Claude Code CLI",
                executable_name: "claude",
                missing_message: "Chưa cài Claude Code CLI: npm i -g @anthropic-ai/claude-code",
                configured_executable: request.provider.executable.clone(),
                folder: &request.folder,
                args,
                prompt,
                watchdog_minutes: None,
            },
            events.clone(),
            cancel.clone(),
            |line| {
                super::limits::record_claude(&request.provider.id, line);
                let mut parsed = parser
                    .lock()
                    .expect("Claude parser mutex poisoned")
                    .parse(line);
                for event in &mut parsed {
                    if event.name == "session" {
                        if let Some(id) = event.data["session_id"].as_str() {
                            event.data = serde_json::json!({"session_id":format!("{prefix}{id}")});
                        }
                    }
                }
                parsed
            },
        )
        .await;
        if missing_result_event(
            parser
                .lock()
                .expect("Claude parser mutex poisoned")
                .result_seen,
            cancel.is_cancelled(),
        ) {
            let _ = events
                .send(ProviderEvent::text(
                    "error",
                    "Claude Code CLI kết thúc đột ngột (không có kết quả)",
                ))
                .await;
        }
        result
    }
}

fn permission_args(permission: Permission) -> Vec<OsString> {
    let mode = match permission {
        Permission::Read => "plan",
        Permission::Write | Permission::Full => "acceptEdits",
    };
    let mut args = vec!["--permission-mode".into(), mode.into()];
    if permission == Permission::Full {
        // --allowedTools <tools...> accepts separate tool names.
        args.extend(["--allowedTools".into(), "Bash".into(), "PowerShell".into()]);
    }
    args
}

fn missing_result_event(result_seen: bool, cancelled: bool) -> bool {
    !result_seen && !cancelled
}

#[derive(Default)]
struct ClaudeParser {
    permission: Permission,
    denial_hint_seen: bool,
    result_seen: bool,
    partial_seen: bool,
    delta_seen: bool,
    /// True when the next text belongs to a NEW assistant message / text block, so it must be separated from
    /// the text already shown (otherwise "...câu cuối.Câu đầu..." glue together across tool calls).
    new_text_segment: bool,
}

impl ClaudeParser {
    fn text_delta(&mut self, text: &str) -> ProviderEvent {
        let separator = if self.delta_seen && self.new_text_segment {
            "\n\n"
        } else {
            ""
        };
        self.new_text_segment = false;
        self.delta_seen = true;
        ProviderEvent::text("delta", format!("{separator}{text}"))
    }

    fn parse(&mut self, line: &str) -> Vec<ProviderEvent> {
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
            "system" => {}
            "user" => {
                if self.permission != Permission::Full && !self.denial_hint_seen {
                    let denied = value["message"]["content"]
                        .as_array()
                        .is_some_and(|blocks| {
                            blocks.iter().any(|block| {
                                if block["type"] != "tool_result" || block["is_error"] != true {
                                    return false;
                                }
                                let content = block["content"].to_string().to_lowercase();
                                content.contains("requires approval")
                                    || content.contains("haven't granted")
                            })
                        });
                    if denied {
                        self.denial_hint_seen = true;
                        events.push(ProviderEvent::text(
                            "log",
                            "Lệnh bị chặn: chọn Quyền › Toàn quyền (chạy lệnh) để cho phép AI chạy lệnh.",
                        ));
                    }
                }
            }
            "assistant" => {
                if let Some(blocks) = value["message"]["content"].as_array() {
                    for block in blocks {
                        match block["type"].as_str().unwrap_or("") {
                            "text" if !self.partial_seen => {
                                if let Some(text) =
                                    block["text"].as_str().filter(|text| !text.is_empty())
                                {
                                    events.push(self.text_delta(text));
                                }
                            }
                            "tool_use" => {
                                events.push(ProviderEvent::text("log", summarize_tool_use(block)));
                                // A skill/plugin run shows as its own line in the reply, free of model tokens.
                                if let Some(skill) = skill_name(block) {
                                    let lead = if self.delta_seen { "\n\n" } else { "" };
                                    events.push(ProviderEvent::text(
                                        "delta",
                                        format!("{lead}✦ {skill}\n\n"),
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                self.partial_seen = false;
                // The message is complete: whatever text comes next starts a new message.
                self.new_text_segment = true;
            }
            "stream_event" => {
                let inner = value["event"]["type"].as_str().unwrap_or("");
                let starts_text_block = inner == "content_block_start"
                    && value["event"]["content_block"]["type"].as_str() == Some("text");
                if inner == "message_start" || starts_text_block {
                    self.new_text_segment = true;
                }
                if value["event"]["type"].as_str() == Some("content_block_delta")
                    && value["event"]["delta"]["type"].as_str() == Some("text_delta")
                {
                    if let Some(text) = value["event"]["delta"]["text"]
                        .as_str()
                        .filter(|text| !text.is_empty())
                    {
                        self.partial_seen = true;
                        events.push(self.text_delta(text));
                    }
                }
            }
            "rate_limit_event" => {
                let info = &value["rate_limit_info"];
                let status = info["status"].as_str().unwrap_or("unknown");
                let reset = info
                    .get("resetsAt")
                    .filter(|value| !value.is_null())
                    .or_else(|| info.get("resets_at").filter(|value| !value.is_null()));
                if !matches!(status.to_ascii_lowercase().as_str(), "allowed" | "ok")
                    || reset.is_some()
                {
                    let detail = reset
                        .map(|time| {
                            let time = time
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| time.to_string());
                            format!("Claude đang gần/chạm giới hạn lượt dùng — reset lúc {time}")
                        })
                        .unwrap_or_else(|| "Claude đang gần/chạm giới hạn lượt dùng".to_owned());
                    events.push(ProviderEvent::text("log", detail));
                }
            }
            "result" => {
                self.result_seen = true;
                let subtype = value["subtype"].as_str().unwrap_or("unknown");
                let is_error = value["is_error"].as_bool().unwrap_or(false);
                let result_text = value["result"].as_str().unwrap_or_default();
                if subtype != "success" || is_error {
                    let label = match subtype {
                        "error_max_turns" => "Claude dừng vì hết số lượt (max turns)".to_owned(),
                        "error_during_execution" => "Claude gặp lỗi khi chạy".to_owned(),
                        "success" if is_error => "Claude Code CLI trả về lỗi".to_owned(),
                        other => other.to_owned(),
                    };
                    let message = if result_text.is_empty() {
                        label
                    } else {
                        format!("{label}: {result_text}")
                    };
                    events.push(ProviderEvent::text("error", message));
                } else if !self.delta_seen && !result_text.is_empty() {
                    events.push(ProviderEvent::text("delta", result_text.to_owned()));
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
}

/// Name of the skill when a `tool_use` block is Claude Code's `Skill` tool.
fn skill_name(block: &Value) -> Option<&str> {
    (block["name"] == "Skill")
        .then(|| block["input"]["skill"].as_str())
        .flatten()
        .filter(|name| !name.is_empty())
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

#[cfg(test)]
fn parse_claude_line(line: &str) -> Vec<ProviderEvent> {
    ClaudeParser::default().parse(line)
}

#[cfg(test)]
mod tests {
    use super::{parse_claude_line, ClaudeParser};
    use crate::chat::Permission;

    #[test]
    fn permission_arguments_only_allow_commands_for_full() {
        for (permission, mode) in [
            (Permission::Read, "plan"),
            (Permission::Write, "acceptEdits"),
            (Permission::Full, "acceptEdits"),
        ] {
            let args = super::permission_args(permission);
            let args: Vec<_> = args.iter().map(|arg| arg.to_str().unwrap()).collect();
            assert_eq!(&args[..2], &["--permission-mode", mode]);
            if permission == Permission::Full {
                assert_eq!(&args[2..], &["--allowedTools", "Bash", "PowerShell"]);
            } else {
                assert_eq!(args.len(), 2);
            }
            assert!(!args.iter().any(|arg| arg.contains("bypassPermissions")));
        }
    }

    #[test]
    fn command_denial_hint_is_emitted_once_per_run_except_full() {
        for permission in [Permission::Read, Permission::Write, Permission::Full] {
            for content in [
                serde_json::json!("Bash requires approval"),
                serde_json::json!([{"type": "text", "text": "You haven't granted PowerShell"}]),
            ] {
                let mut parser = ClaudeParser {
                    permission,
                    ..Default::default()
                };
                let line = serde_json::json!({"type":"user","message":{"content":[{
                    "type":"tool_result", "is_error":true, "content":content
                }]}})
                .to_string();
                let first = parser.parse(&line);
                assert_eq!(first.len(), usize::from(permission != Permission::Full));
                if permission != Permission::Full {
                    assert_eq!(first[0].name, "log");
                    assert_eq!(first[0].data["text"],
                        "Lệnh bị chặn: chọn Quyền › Toàn quyền (chạy lệnh) để cho phép AI chạy lệnh.");
                }
                assert!(parser.parse(&line).is_empty());
            }
        }
        let mut parser = ClaudeParser::default();
        for (kind, error, content) in [
            ("tool_result", false, "requires approval"),
            ("text", true, "requires approval"),
            ("tool_result", true, "file missing"),
        ] {
            let line = serde_json::json!({"type":"user","message":{"content":[{
                "type":kind,"is_error":error,"content":content
            }]}})
            .to_string();
            assert!(parser.parse(&line).is_empty());
        }
    }

    #[test]
    fn maps_claude_result_subtypes_and_returns_success_text() {
        let max_turns =
            parse_claude_line(r#"{"type":"result","subtype":"error_max_turns","result":"limit"}"#);
        assert_eq!(
            max_turns[0].data["text"],
            "Claude dừng vì hết số lượt (max turns): limit"
        );
        let other = parse_claude_line(
            r#"{"type":"result","subtype":"error_max_budget_usd","result":"budget"}"#,
        );
        assert!(other[0].data["text"]
            .as_str()
            .unwrap()
            .contains("error_max_budget_usd"));
        let success =
            parse_claude_line(r#"{"type":"result","subtype":"success","result":"answer"}"#);
        assert_eq!(success[0].name, "delta");
    }
    #[test]
    fn partial_text_suppresses_full_assistant_text_and_result_duplicate() {
        let mut parser = ClaudeParser::default();
        let partial = parser.parse(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"partial"}}}"#);
        assert_eq!(partial.len(), 1);
        let complete = parser.parse(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"partial"}]}}"#,
        );
        assert!(complete.is_empty());
        let result = parser.parse(r#"{"type":"result","subtype":"success","result":"partial"}"#);
        assert!(result.is_empty());
    }
    #[test]
    fn detects_missing_result_event_with_default_state() {
        assert!(super::missing_result_event(false, false));
        assert!(!super::missing_result_event(true, false));
        assert!(!super::missing_result_event(false, true));
        let mut parser = ClaudeParser::default();
        parser.parse(r#"{"type":"result","subtype":"success","result":"ok"}"#);
        assert!(parser.result_seen);
    }
}

#[cfg(test)]
mod separator_tests {
    use super::ClaudeParser;

    fn deltas(events: Vec<crate::providers::ProviderEvent>) -> String {
        events
            .iter()
            .filter(|event| event.name == "delta")
            .map(|event| event.data["text"].as_str().unwrap_or_default())
            .collect()
    }

    #[test]
    fn text_from_separate_assistant_messages_is_not_glued_together() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        // First assistant message (streamed), then a tool call, then a second message with more text.
        for line in [
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Câu đầu."}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Câu đầu."}]}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Câu sau."}}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "Câu đầu.\n\nCâu sau.");
    }

    #[test]
    fn a_message_without_text_adds_no_blank_gap() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"A"}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"A"},{"type":"tool_use","name":"Read","input":{}}]}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "A");
    }

    #[test]
    fn a_skill_run_shows_as_its_own_line() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Trước"}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Trước"},{"type":"tool_use","name":"Skill","input":{"skill":"ponytail"}}]}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "Trước\n\n✦ ponytail\n\n");
    }
}
