use crate::chat::Permission;
use crate::providers::{cli_support, ProviderEvent};
use serde_json::{json, Value};

/// What a tool step is about, as (agy allow-rule, text shown to the user): a shell command or an MCP tool.
pub(super) fn tool_target(step: &Value) -> Option<(String, String)> {
    let parameters = &step["tool_info"]["parameters"];
    if let Some(command) = parameters["CommandLine"].as_str() {
        return Some((format!("command({command})"), command.to_owned()));
    }
    let (server, tool) = (parameters["ServerName"].as_str()?, parameters["ToolName"].as_str()?);
    Some((format!("mcp({server}/{tool})"), format!("MCP {server}: {tool}")))
}

#[derive(Default)]
pub(super) struct Parser {
    pub(super) permission: Permission,
    pub(super) result_seen: bool,
    pub(super) delta_seen: bool,
    pub(super) conversation: Option<String>,
    /// Commands the mode refused this turn: the run asks the user about them instead of offering a rerun.
    pub(super) denied: Vec<(String, String)>,
    /// Commands begun and not yet finished: agy sometimes refuses a command without ever reporting the failed step.
    pub(super) started: Vec<(String, String)>,
    /// A file write was refused by the Read rule.
    pub(super) write_blocked: bool,
    /// `usage.input_tokens` of the result: grows with the conversation.
    pub(super) input_tokens: Option<u64>,
    /// Step index of the last text, so text from the next step starts a new paragraph.
    pub(super) last_text_step: Option<u64>,
}

impl Parser {
    pub(super) fn parse(&mut self, line: &str) -> Vec<ProviderEvent> {
        if line.trim().is_empty() {
            return Vec::new();
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return vec![ProviderEvent::text("log", format!("Antigravity output: {}", cli_support::one_line(line, 240)))];
        };
        match value["event"].as_str().unwrap_or("") {
            "init" => value["conversation_id"]
                .as_str()
                .filter(|id| cli_support::safe_session_id(id))
                .map(|id| {
                    self.conversation = Some(id.to_owned());
                    vec![ProviderEvent { name: "session", data: json!({"session_id": id}) }]
                })
                .unwrap_or_default(),
            "step_update" => self.step(&value["step_update"]),
            "result" => self.result(&value["result"]),
            _ => Vec::new(),
        }
    }

    fn step(&mut self, step: &Value) -> Vec<ProviderEvent> {
        match step["step_type"].as_str().unwrap_or("") {
            "agent_response" => {
                let Some(text) = step["text_delta"].as_str().filter(|text| !text.is_empty()) else { return Vec::new() };
                let index = step["step_index"].as_u64();
                let separator = if self.delta_seen && self.last_text_step != index { "\n\n" } else { "" };
                self.delta_seen = true;
                self.last_text_step = index;
                vec![ProviderEvent::text("delta", format!("{separator}{text}"))]
            }
            "tool" if step["state"] == "DONE" => {
                if let Some(target) = tool_target(step) { self.started.retain(|known| *known != target); }
                Vec::new()
            }
            "tool" if step["state"] == "ERROR" => {
                let refused = step["tool_info"]["error"]["message"].as_str().is_some_and(|message| message.contains("permission"));
                if refused && step["tool_info"]["parameters"]["Path"].is_string() { self.write_blocked = true; }
                if let (true, Some(target)) = (refused, tool_target(step)) {
                    if !self.denied.contains(&target) { self.denied.push(target); }
                }
                Vec::new()
            }
            "tool" if step["state"] == "ACTIVE" => {
                if let Some(target) = tool_target(step) { self.started.push(target); }
                vec![ProviderEvent::text("log", format!("Công cụ: {}", step["tool_name"].as_str().unwrap_or("?")))]
            }
            _ => Vec::new(),
        }
    }

    fn result(&mut self, result: &Value) -> Vec<ProviderEvent> {
        self.result_seen = true;
        let mut events = Vec::new();
        let response = result["response"].as_str().unwrap_or_default();
        if result["status"] != "SUCCESS" {
            let reason = result["error"].as_str().filter(|text| !text.is_empty()).unwrap_or(response);
            let status = result["status"].as_str().unwrap_or("UNKNOWN");
            events.push(ProviderEvent::text("error", if reason.is_empty() { format!("Antigravity dừng với trạng thái {status}") } else { format!("Antigravity {status}: {reason}") }));
        } else if !self.delta_seen && !response.is_empty() {
            events.push(ProviderEvent::text("delta", response.to_owned()));
        }
        if result["denied_actions"].as_array().is_some_and(|actions| !actions.is_empty()) && self.denied.is_empty() {
            self.denied = std::mem::take(&mut self.started);
        }
        // A tool that needed a permission this mode lacks: the chat offers a rerun one level up.
        let blocked = result["denied_actions"].as_array().is_some_and(|actions| !actions.is_empty()) && self.denied.is_empty();
        if (blocked || self.write_blocked) && self.permission != Permission::Full && self.denied.is_empty() {
            let need = if self.permission == Permission::Read { "write" } else { "full" };
            events.push(ProviderEvent { name: "permission_denied", data: json!({ "need": need }) });
        }
        let usage = &result["usage"];
        self.input_tokens = usage["input_tokens"].as_u64();
        let normalized = json!({
            "input_tokens": usage["input_tokens"], "output_tokens": usage["output_tokens"],
            "cached_input_tokens": usage["cache_read_tokens"], "thinking_tokens": usage["thinking_tokens"],
        });
        if let Some(line) = cli_support::token_usage_log(&normalized, None) {
            events.push(ProviderEvent::text("log", line));
        }
        events
    }
}
