//! Antigravity CLI (`agy`): one headless turn per message, JSON in and out (`--input-format/--output-format stream-json`).
use super::{cli_support, Provider, ProviderEvent, ProviderRequest};
use crate::chat::Permission;
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct AntigravityCliProvider;

/// Session ids saved by another source or folder must never be handed to `--conversation`.
fn session_prefix(request: &ProviderRequest) -> String {
    let text = format!("agy:{}:{}:{}", request.provider.id, request.provider.executable, request.folder);
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let key = hash.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("mutis-agy-{key}:")
}

/// Clean per-source home (`<data>/antigravity/<id>`): agy keeps its chats, plugins, settings and MCP list under `~/.gemini`,
/// so pointing USERPROFILE/HOME here keeps them apart from the user's own agy. The login lives in the OS keyring and still works.
pub fn clean_home(provider_id: &str) -> Result<std::path::PathBuf> {
    let id: String = provider_id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect();
    let dir = crate::core::paths::data_dir(&crate::core::config::config_file_path()?)?.join("antigravity").join(id);
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// AI Hub's MCP list (and CodeGraph) becomes the clean home's `mcp_config.json`, rewritten every turn.
fn write_mcp_config(home: &std::path::Path, servers: &[crate::config::McpServerConfig]) -> Result<()> {
    let list: serde_json::Map<String, Value> = servers
        .iter()
        .filter(|s| s.enabled)
        .map(|s| (s.name.clone(), json!({"command": s.command, "args": s.args, "env": s.env})))
        .collect();
    let dir = home.join(".gemini").join("config");
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("mcp_config.json"), serde_json::to_vec_pretty(&json!({"mcpServers": list}))?)?;
    Ok(())
}

/// shortcut: Read maps to agy's `plan` mode, which also writes its plan file under ~/.gemini/antigravity-cli; no stricter read-only mode exists.
fn mode(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "plan",
        Permission::Write => "accept-edits",
        Permission::Full => "auto",
    }
}

#[async_trait]
impl Provider for AntigravityCliProvider {
    async fn run(&self, request: ProviderRequest, events: mpsc::Sender<ProviderEvent>, cancel: CancellationToken) -> Result<()> {
        if !request.model.is_empty() && !cli_support::valid_model(&request.model) {
            bail!("Tên model chỉ được chứa chữ, số, ., _, :, / và -");
        }
        let prefix = session_prefix(&request);
        let native = request.session_id.as_deref().and_then(|id| id.strip_prefix(&prefix)).map(str::to_owned);
        // `--print=` leaves the prompt empty so it arrives on stdin as a stream-json message (no command-line length limit).
        let mut args: Vec<OsString> = ["--print=", "--input-format", "stream-json", "--output-format", "stream-json", "--mode", mode(request.permission)]
            .map(OsString::from)
            .into();
        if !request.model.is_empty() {
            args.extend([OsString::from("--model"), OsString::from(&request.model)]);
        }
        if let Some(id) = native.as_deref() {
            if !cli_support::safe_session_id(id) {
                bail!("Antigravity conversation id không hợp lệ");
            }
            args.extend([OsString::from("--conversation"), OsString::from(id)]);
        }
        let mut prompt = cli_support::compose_prompt_for(&request.messages, native.is_some(), "antigravity_cli");
        if let Some(directory) = super::latest_chat_attachment_dir(&request.messages) {
            let paths = super::latest_image_paths(&request.messages, &directory)?;
            args.extend([OsString::from("--add-dir"), directory.as_os_str().to_owned()]);
            if !paths.is_empty() {
                prompt.push_str("\n\n");
                for path in paths {
                    prompt.push_str(&format!("Ảnh đính kèm: {}\n", path.display()));
                }
                prompt.push_str("Hãy dùng công cụ đọc tệp để xem ảnh trên.");
            }
        }
        let home = clean_home(&request.provider.id)?;
        write_mcp_config(&home, &request.mcp)?;
        let message = format!("{}\n", json!({"event": "user", "message": {"content": prompt}}));
        let parser = std::sync::Mutex::new(Parser { permission: request.permission, ..Default::default() });
        let result = cli_support::run_jsonl(
            cli_support::JsonlCommand {
                config_dir: Some(home),
                cli_name: "Antigravity CLI",
                executable_name: "agy",
                missing_message: "Chưa cài Antigravity CLI: irm https://antigravity.google/cli/install.ps1 | iex",
                configured_executable: request.provider.executable.clone(),
                folder: &request.folder,
                args,
                prompt: message,
                watchdog_minutes: None,
            },
            events.clone(),
            cancel.clone(),
            |line| {
                let mut parsed = parser.lock().expect("Antigravity parser mutex poisoned").parse(line);
                for event in &mut parsed {
                    if event.name == "session" {
                        if let Some(id) = event.data["session_id"].as_str() {
                            event.data = json!({"session_id": format!("{prefix}{id}")});
                        }
                    }
                }
                parsed
            },
        )
        .await;
        if !parser.lock().expect("Antigravity parser mutex poisoned").result_seen && !cancel.is_cancelled() {
            let _ = events.send(ProviderEvent::text("error", "Antigravity CLI kết thúc đột ngột (không có kết quả)")).await;
        }
        result
    }
}

#[derive(Default)]
struct Parser {
    permission: Permission,
    result_seen: bool,
    delta_seen: bool,
    /// Step index of the last text, so text from the next step starts a new paragraph.
    last_text_step: Option<u64>,
}

impl Parser {
    fn parse(&mut self, line: &str) -> Vec<ProviderEvent> {
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
                .map(|id| vec![ProviderEvent { name: "session", data: json!({"session_id": id}) }])
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
            "tool" if step["state"] == "ACTIVE" => {
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
        // A tool that needed a permission this mode lacks: the chat offers a rerun one level up.
        if result["denied_actions"].as_array().is_some_and(|actions| !actions.is_empty()) && self.permission != Permission::Full {
            let need = if self.permission == Permission::Read { "write" } else { "full" };
            events.push(ProviderEvent { name: "permission_denied", data: json!({ "need": need }) });
        }
        let usage = &result["usage"];
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

#[cfg(test)]
mod tests {
    use super::*;

    fn run(lines: &[&str]) -> Vec<(String, String)> {
        let mut parser = Parser::default();
        lines.iter().flat_map(|line| parser.parse(line)).map(|e| (e.name.to_owned(), e.data.to_string())).collect()
    }

    #[test]
    fn streams_text_separates_steps_and_reports_tokens() {
        let out = run(&[
            r#"{"event":"init","conversation_id":"abc-1"}"#,
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"Một"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"view_file"}}"#,
            r#"{"event":"step_update","step_update":{"step_index":3,"state":"ACTIVE","step_type":"agent_response","text_delta":"Hai"}}"#,
            r#"{"event":"result","result":{"status":"SUCCESS","response":"Một Hai","usage":{"input_tokens":10,"output_tokens":4,"thinking_tokens":2,"cache_read_tokens":0}}}"#,
        ]);
        let text = |name: &str| out.iter().filter(|(n, _)| n == name).map(|(_, d)| d.as_str()).collect::<Vec<_>>();
        assert_eq!(text("session"), [r#"{"session_id":"abc-1"}"#]);
        assert_eq!(text("delta"), [r#"{"text":"Một"}"#, r#"{"text":"\n\nHai"}"#]);
        assert!(text("log").iter().any(|log| log.contains("view_file")));
        assert!(text("log").iter().any(|log| log.contains("vào 10")));
        assert!(text("error").is_empty());
    }

    #[test]
    fn failed_result_is_an_error_and_success_without_stream_falls_back_to_the_response() {
        let failed = run(&[r#"{"event":"result","result":{"status":"ERROR","error":"authentication required"}}"#]);
        assert!(failed.iter().any(|(n, d)| n == "error" && d.contains("authentication required")));
        let plain = run(&[r#"{"event":"result","result":{"status":"SUCCESS","response":"ok"}}"#]);
        assert!(plain.iter().any(|(n, d)| n == "delta" && d.contains("ok")));
    }

    /// Real `agy` call (uses the signed-in account and a little quota): `cargo test antigravity_live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn antigravity_live_turn_streams_text_and_resumes_the_conversation() {
        let folder = std::env::temp_dir().join("ai-hub-agy-live");
        std::fs::create_dir_all(&folder).unwrap();
        let ask = |text: &str, session: Option<String>| ProviderRequest {
            provider: crate::config::ProviderConfig { id: "antigravity-cli".into(), kind: "antigravity_cli".into(), ..Default::default() },
            model: "gemini-4-neon-low".into(),
            folder: folder.to_string_lossy().into_owned(),
            permission: Permission::Read,
            effort: super::super::Effort::Default,
            messages: vec![serde_json::from_value(json!({"role": "user", "content": text})).unwrap()],
            session_id: session,
            mutis: crate::config::Config::seeded().mutis,
            mcp: Vec::new(),
            write_scope: Vec::new(),
            checker_session_id: None,
        };
        let mut session = None;
        let mut replies = Vec::new();
        for text in ["Remember the word PINEAPPLE. Reply only: noted", "What word did I ask you to remember? Reply with that word only."] {
            let (tx, mut rx) = mpsc::channel(256);
            let request = ask(text, session.clone());
            let run = tokio::spawn(async move { AntigravityCliProvider.run(request, tx, CancellationToken::new()).await });
            let mut reply = String::new();
            while let Some(event) = rx.recv().await {
                println!("{} {}", event.name, event.data);
                match event.name {
                    "delta" => reply.push_str(event.data["text"].as_str().unwrap_or_default()),
                    "session" => session = event.data["session_id"].as_str().map(str::to_owned),
                    "error" => panic!("error event: {}", event.data),
                    _ => {}
                }
            }
            run.await.unwrap().unwrap();
            replies.push(reply);
        }
        assert!(session.is_some_and(|id| id.starts_with("mutis-agy-")));
        assert!(replies[1].to_uppercase().contains("PINEAPPLE"), "second turn forgot the first: {replies:?}");
    }

    #[test]
    fn denied_actions_offer_one_level_up_except_in_full_mode() {
        let line = r#"{"event":"result","result":{"status":"SUCCESS","response":"","denied_actions":[{"action":"command","display_name":"Bash"}]}}"#;
        for (permission, need) in [(Permission::Read, Some("write")), (Permission::Write, Some("full")), (Permission::Full, None)] {
            let events = Parser { permission, ..Default::default() }.parse(line);
            let denied = events.iter().find(|e| e.name == "permission_denied");
            assert_eq!(denied.map(|e| e.data["need"].as_str().unwrap()), need);
        }
        assert!(run(&[r#"{"event":"result","result":{"status":"SUCCESS","response":"ok","denied_actions":[]}}"#]).iter().all(|(n, _)| n != "permission_denied"));
    }

    #[test]
    fn permissions_map_to_agy_modes() {
        assert_eq!(mode(Permission::Read), "plan");
        assert_eq!(mode(Permission::Write), "accept-edits");
        assert_eq!(mode(Permission::Full), "auto");
    }
}
