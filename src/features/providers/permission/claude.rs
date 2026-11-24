use crate::chat::Permission;
use crate::providers::claude_cli::summarize_tool_use;
use crate::providers::cli_support;
use serde_json::Value;
use std::ffi::OsString;

/// Tools that only look: never worth asking about.
const LOOK_ONLY: [&str; 6] = ["Read", "Glob", "Grep", "LS", "TodoWrite", "WebSearch"];

fn permission_reply(request_id: &str, allow: bool, input: &Value) -> String {
    let body = if allow {
        serde_json::json!({"behavior": "allow", "updatedInput": input})
    } else {
        serde_json::json!({"behavior": "deny", "message": "The user declined this action."})
    };
    serde_json::json!({"type": "control_response", "response": {"subtype": "success", "request_id": request_id, "response": body}}).to_string()
}

/// Claude asks before each tool the permission mode does not cover (`--permission-prompt-tool stdio`): the user answers from the chat.
pub(crate) fn interaction(permission: Permission) -> cli_support::Interact {
    Box::new(move |line| {
        let value: Value = serde_json::from_str(line).ok()?;
        if value["type"] != "control_request" || value["request"]["subtype"] != "can_use_tool" {
            return None;
        }
        let id = value["request_id"].as_str()?.to_owned();
        let tool = value["request"]["tool_name"].as_str().unwrap_or("");
        let input = value["request"]["input"].clone();
        if LOOK_ONLY.contains(&tool) {
            return Some(cli_support::Reaction::Reply(permission_reply(&id, true, &input)));
        }
        // Questions and plan approval have no one to answer them in a headless run.
        if matches!(tool, "AskUserQuestion" | "ExitPlanMode" | "EnterPlanMode") {
            return Some(cli_support::Reaction::Reply(permission_reply(&id, false, &input)));
        }
        let shown = match input["file_path"].as_str() {
            Some(path) => format!("{tool}: {}", cli_support::one_line(path, 220)),
            None => summarize_tool_use(&serde_json::json!({"name": tool, "input": input})),
        };
        let need = if permission == Permission::Read { "write" } else { "full" };
        Some(cli_support::Reaction::Ask { shown, need, reply: Box::new(move |allow| permission_reply(&id, allow, &input)) })
    })
}

pub(crate) fn permission_args(permission: Permission) -> Vec<OsString> {
    let mode = match permission {
        Permission::Read => "manual",
        Permission::Write | Permission::Full => "acceptEdits",
    };
    let mut args = vec!["--permission-mode".into(), mode.into()];
    if permission == Permission::Full {
        // --allowedTools <tools...> accepts separate tool names.
        args.extend(["--allowedTools".into(), "Bash".into(), "PowerShell".into()]);
    }
    args
}
