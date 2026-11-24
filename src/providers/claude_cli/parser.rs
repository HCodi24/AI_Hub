use crate::chat::Permission;
use crate::providers::{cli_support, codex, ProviderEvent};
use serde_json::Value;

/// Claude's `total_cost_usd` is the running total of the whole session, not of the turn. The turn's own cost is the
/// difference from the previous total, kept per session while the app runs.
/// shortcut: after an app restart the first resumed turn has no baseline, so its cost is left out instead of showing the
/// whole session's total; persist the total per chat if that gap matters.
pub(super) fn turn_cost(session: Option<&str>, total: Option<f64>, resumed: bool) -> Option<f64> {
    static TOTALS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, f64>>> = std::sync::OnceLock::new();
    let (session, total) = (session?, total?);
    let mut totals = TOTALS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    let before = totals.insert(session.to_owned(), total);
    match before {
        Some(before) => Some((total - before).max(0.0)),
        None => (!resumed).then_some(total),
    }
}

/// Input tokens of the last model call of a turn = how big the session is now.
pub(super) fn context_size(usage: &Value) -> Option<u64> {
    let last = usage["iterations"].as_array()?.last()?;
    Some(["input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"].iter().filter_map(|key| last[*key].as_u64()).sum())
}

/// shortcut: a Write into a `plans` folder or ExitPlanMode is read as "it wanted to act but plan mode stopped it"; a plan written for a pure question would show the card too.
fn plan_instead_of_acting(block: &serde_json::Value) -> bool {
    let path = block["input"]["file_path"].as_str().unwrap_or("").replace('\\', "/");
    block["name"] == "ExitPlanMode" || (block["name"] == "Write" && path.contains("/plans/"))
}

#[derive(Default)]
pub(super) struct ClaudeParser {
    pub(super) permission: Permission,
    /// The turn continues a saved session, whose `total_cost_usd` already holds the cost of the earlier turns.
    pub(super) resumed: bool,
    pub(super) denial_hint_seen: bool,
    pub(super) result_seen: bool,
    pub(super) partial_seen: bool,
    pub(super) delta_seen: bool,
    /// True when the next text belongs to a NEW assistant message / text block, so it must be separated from
    /// the text already shown (otherwise "...câu cuối.Câu đầu..." glue together across tool calls).
    pub(super) new_text_segment: bool,
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

    pub(super) fn parse(&mut self, line: &str) -> Vec<ProviderEvent> {
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
                        // One level up only: the chat offers a rerun at that level, never straight to full.
                        let need = if self.permission == Permission::Read { "write" } else { "full" };
                        events.push(ProviderEvent { name: "permission_denied", data: serde_json::json!({ "need": need }) });
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
                                // Plan mode never reports an error: it writes its plan file (or asks to leave plan mode) instead of acting.
                                if plan_instead_of_acting(block) && self.permission == Permission::Read && !self.denial_hint_seen {
                                    self.denial_hint_seen = true;
                                    events.push(ProviderEvent { name: "permission_denied", data: serde_json::json!({ "need": "write" }) });
                                }
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
                // shortcut: plan mode often just says so in its reply (no tool call, no error); matching "plan mode" in the final text can also hit a reply that merely mentions it.
                if self.permission == Permission::Read && !self.denial_hint_seen && result_text.to_lowercase().contains("plan mode") {
                    self.denial_hint_seen = true;
                    events.push(ProviderEvent { name: "permission_denied", data: serde_json::json!({ "need": "write" }) });
                }
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
                if let (Some(id), Some(size)) = (value["session_id"].as_str(), context_size(&value["usage"])) {
                    codex::compact::note_size(id, size);
                }
                let cost = turn_cost(value["session_id"].as_str(), value["total_cost_usd"].as_f64(), self.resumed).map(Value::from);
                if let Some(usage) = cli_support::token_usage_log(&value["usage"], cost.as_ref())
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

pub(super) fn summarize_tool_use(block: &Value) -> String {
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
pub(super) fn parse_claude_line(line: &str) -> Vec<ProviderEvent> {
    ClaudeParser::default().parse(line)
}
