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

pub(crate) const SIGNED_OUT_FILE: &str = ".mutis-signed-out";

/// Session ids saved by another source or folder must never be handed to `--conversation`.
pub(crate) fn session_prefix(provider: &crate::config::ProviderConfig, folder: &str, settings: &crate::config::AntigravityConfig) -> String {
    let mut text = format!("agy:{}:{}:{}", provider.id, provider.executable, folder);
    if settings.project_skills || settings.project_instructions {
        text.push_str(&format!(":project-context:{}:{}", settings.project_skills, settings.project_instructions));
    }
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let key = hash.as_ref()[..8].iter().map(|b| format!("{b:02x}")).collect::<String>();
    format!("mutis-agy-{key}:")
}

/// Clean per-source home (`<data>/antigravity/<id>`): agy keeps its chats, plugins, settings and MCP list under `~/.gemini`,
/// so pointing USERPROFILE/HOME here keeps them apart from the user's own agy. The login lives in the OS keyring and still works.
/// Keeps the home's login mode in step with the source: a web source marks it "web" (see `cli_support::agy_environment`).
pub fn sync_login_mode(provider: &crate::config::ProviderConfig) {
    if provider.kind == "antigravity_cli" {
        if let Ok(home) = clean_home(&provider.id) {
            cli_support::set_auth_origin(&home, provider.web_login.then_some("web"));
        }
    }
}

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

/// The app's three levels on agy's modes. agy's `plan` mode only asks in words, so Read is `accept-edits` with every file write denied
/// by rule (see `permission_rules`); commands still ask. Write allows edits; Full is `auto`.
fn mode(permission: Permission) -> &'static str {
    match permission {
        Permission::Read => "accept-edits",
        Permission::Write => "accept-edits",
        Permission::Full => "auto",
    }
}

/// agy's `--settings` permission rules for one turn: the app's CodeGraph is fine, `extra` are what the user just allowed, and Read denies
/// every file write (agy checks file edits, too, as `write_file`). No command is pre-allowed: a wildcard rule such as `command(dir *)` also
/// lets `dir . > a.txt` through, and agy already lets its own read-only tools and commands (list_dir, view_file, git status) pass.
fn permission_rules(permission: Permission, extra: &[String]) -> Value {
    let mut allow = vec!["mcp(codegraph/codegraph_explore)".to_owned()];
    allow.extend(extra.iter().cloned());
    let deny: &[&str] = if permission == Permission::Read { &["write_file(*)"] } else { &[] };
    json!({"permissions": {"allow": allow, "deny": deny}})
}

#[async_trait]
impl Provider for AntigravityCliProvider {
    async fn run(&self, request: ProviderRequest, events: mpsc::Sender<ProviderEvent>, cancel: CancellationToken) -> Result<()> {
        if !request.model.is_empty() && !cli_support::valid_model(&request.model) {
            bail!("Tên model chỉ được chứa chữ, số, ., _, :, / và -");
        }
        let config: crate::config::Config = serde_json::from_slice(&std::fs::read(crate::config::config_file_path()?)?)?;
        let settings = &config.antigravity;
        let prefix = session_prefix(&request.provider, &request.folder, settings);
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
        }
        let mut prompt = cli_support::compose_prompt_for(&request.messages, native.is_some(), "antigravity_cli");
        if native.is_none() {
            let context = super::claude::sources::collect_project_context(
                std::path::Path::new(&request.folder),
                if settings.project_instructions { &["AGENTS.md", "GEMINI.md", ".agents/AGENTS.md", ".agents/GEMINI.md", ".agents/rules"] } else { &[] },
                if settings.project_skills { &[".agents/skills"] } else { &[] },
            )?;
            if !context.is_empty() {
                prompt = format!("{context}\n\n{prompt}");
            }
        }
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
        if home.join(SIGNED_OUT_FILE).try_exists()? {
            bail!("Hãy đăng nhập lại Antigravity CLI trong MutiS");
        }
        write_mcp_config(&home, &request.mcp)?;
        // Past the token budget the conversation is compacted in place (`/compact`), as Codex and Claude threads are.
        if let Some(id) = native.as_deref() {
            let budget = crate::config::config_file_path().map_or(0, |path| super::codex::compact::thread_token_budget(&path));
            if let Some(size) = super::codex::compact::needs_compaction(id, budget) {
                let _ = events.send(ProviderEvent::text("log", format!("Luồng Antigravity đã thêm {size} token; đang nén ngữ cảnh."))).await;
                let note = match compact_session(&request.provider, &home, &request.folder, id, &cancel).await {
                    Ok(()) => "Đã nén ngữ cảnh luồng Antigravity.".to_owned(),
                    Err(error) => format!("Không nén được luồng Antigravity, dùng tiếp như cũ: {error}"),
                };
                let _ = events.send(ProviderEvent::text("log", note)).await;
            }
        }
        let mut message = format!("{}\n", json!({"event": "user", "message": {"content": prompt}}));
        let mut conversation = native.clone();
        // agy cannot be asked in the middle of a turn (headless, it auto-denies): a denied command is put to the user, then the turn
        // is resumed with an allow-rule for exactly that command.
        let mut allowed: Vec<String> = Vec::new();
        let mut round = 0;
        loop {
            let mut round_args = args.clone();
            if let Some(id) = conversation.as_deref() {
                round_args.extend([OsString::from("--conversation"), OsString::from(id)]);
            }
            round_args.extend([OsString::from("--settings"), OsString::from(permission_rules(request.permission, &allowed).to_string())]);
            let parser = std::sync::Mutex::new(Parser { permission: request.permission, ..Default::default() });
            let result = cli_support::run_jsonl(
                cli_support::JsonlCommand {
                    config_dir: Some(home.clone()),
                    cli_name: "Antigravity CLI",
                    executable_name: "agy",
                    missing_message: "Chưa cài Antigravity CLI: irm https://antigravity.google/cli/install.ps1 | iex",
                    configured_executable: request.provider.executable.clone(),
                    folder: &request.folder,
                    args: round_args,
                    prompt: message.clone(),
                    watchdog_minutes: None,
                    interact: None,
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
            let (result_seen, denied, seen_conversation) = {
                let parser = parser.lock().expect("Antigravity parser mutex poisoned");
                if let (Some(id), Some(input)) = (parser.conversation.as_deref(), parser.input_tokens) { note_input(id, input); }
                (parser.result_seen, parser.denied.clone(), parser.conversation.clone())
            };
            if cancel.is_cancelled() || result.is_err() || denied.is_empty() || round == 2 {
                if !result_seen && !cancel.is_cancelled() {
                    let _ = events.send(ProviderEvent::text("error", "Antigravity CLI kết thúc đột ngột (không có kết quả)")).await;
                }
                return result;
            }
            let mut granted = Vec::new();
            for (rule, shown) in denied {
                let Some(yes) = super::approvals::confirm(&events, &cancel, shown.clone()).await else { return Ok(()) };
                let note = if yes { "Bạn đã cho phép bước này một lần." } else { "Bạn đã từ chối bước này." };
                let _ = events.send(ProviderEvent::text("log", note.to_owned())).await;
                if yes { granted.push((rule, shown)); } else {
                    let need = if request.permission == Permission::Read { "write" } else { "full" };
                    let _ = events.send(ProviderEvent { name: "permission_denied", data: json!({ "need": need }) }).await;
                }
            }
            if granted.is_empty() {
                // agy answered nothing after the refusal; say so instead of leaving an empty reply.
                let _ = events.send(ProviderEvent::text("delta", "Bạn đã từ chối nên mình chưa chạy bước đó.".to_owned())).await;
                return result;
            }
            let list = granted.iter().map(|(_, shown)| format!("- {shown}")).collect::<Vec<_>>().join("\n");
            message = format!("{}\n", json!({"event": "user", "message": {"content": format!("The user has now allowed these steps, so do them again and carry on with the task:\n{list}")}}));
            allowed.extend(granted.into_iter().map(|(rule, _)| rule));
            conversation = seen_conversation.or(conversation);
            round += 1;
        }
    }
}

/// What a tool step is about, as (agy allow-rule, text shown to the user): a shell command or an MCP tool.
fn tool_target(step: &Value) -> Option<(String, String)> {
    let parameters = &step["tool_info"]["parameters"];
    if let Some(command) = parameters["CommandLine"].as_str() {
        return Some((format!("command({command})"), command.to_owned()));
    }
    let (server, tool) = (parameters["ServerName"].as_str()?, parameters["ToolName"].as_str()?);
    Some((format!("mcp({server}/{tool})"), format!("MCP {server}: {tool}")))
}

/// agy's `input_tokens` only ever grows (it does not drop after a compaction), so the size the budget is checked against is the growth since
/// the last compaction: `(raw input, raw input right after the last compaction)` per conversation, kept while the app runs.
fn sizes() -> &'static std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>> {
    static SIZES: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<String, (u64, u64)>>> = std::sync::OnceLock::new();
    SIZES.get_or_init(Default::default)
}

fn note_input(conversation: &str, input: u64) {
    let mut sizes = sizes().lock().unwrap_or_else(|e| e.into_inner());
    let base = sizes.get(conversation).map_or(0, |(_, base)| *base);
    sizes.insert(conversation.to_owned(), (input, base));
    super::codex::compact::note_size(conversation, input.saturating_sub(base));
}

/// Compacts a saved conversation with agy's own `/compact` (same conversation id afterwards). Used before a turn past the token budget
/// and by the "Nén ngữ cảnh" button.
pub(crate) async fn compact_session(provider: &crate::config::ProviderConfig, home: &std::path::Path, folder: &str, conversation: &str, cancel: &CancellationToken) -> Result<()> {
    if !cli_support::safe_session_id(conversation) {
        bail!("Antigravity conversation id không hợp lệ");
    }
    let outcome = std::sync::Arc::new(std::sync::Mutex::new(None::<u64>));
    let seen = outcome.clone();
    let (sink, mut drained) = mpsc::channel(64);
    tokio::spawn(async move { while drained.recv().await.is_some() {} });
    cli_support::run_jsonl(
        cli_support::JsonlCommand {
            config_dir: Some(home.to_owned()),
            cli_name: "Antigravity CLI",
            executable_name: "agy",
            missing_message: "Chưa cài Antigravity CLI: irm https://antigravity.google/cli/install.ps1 | iex",
            configured_executable: provider.executable.clone(),
            folder,
            args: ["--print=/compact", "--conversation", conversation, "--output-format", "json"].map(OsString::from).into(),
            prompt: String::new(),
            watchdog_minutes: None,
            interact: None,
        },
        sink,
        cancel.clone(),
        move |line| {
            if let Ok(value) = serde_json::from_str::<Value>(line) {
                if value["status"] == "SUCCESS" { *seen.lock().unwrap_or_else(|e| e.into_inner()) = Some(value["usage"]["input_tokens"].as_u64().unwrap_or(0)); }
            }
            Vec::new()
        },
    )
    .await?;
    let Some(input) = *outcome.lock().unwrap_or_else(|e| e.into_inner()) else { bail!("Antigravity không báo nén thành công") };
    sizes().lock().unwrap_or_else(|e| e.into_inner()).insert(conversation.to_owned(), (input, input));
    super::codex::compact::forget(conversation);
    Ok(())
}

#[derive(Default)]
struct Parser {
    permission: Permission,
    result_seen: bool,
    delta_seen: bool,
    conversation: Option<String>,
    /// Commands the mode refused this turn: the run asks the user about them instead of offering a rerun.
    denied: Vec<(String, String)>,
    /// Commands begun and not yet finished: agy sometimes refuses a command without ever reporting the failed step.
    started: Vec<(String, String)>,
    /// A file write was refused by the Read rule.
    write_blocked: bool,
    /// `usage.input_tokens` of the result: grows with the conversation.
    input_tokens: Option<u64>,
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
        // A refused command is asked about by the run itself, so the result adds no rerun card.
        let mut parser = Parser { permission: Permission::Read, ..Default::default() };
        parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ERROR","tool_info":{"parameters":{"CommandLine":"echo hi > a.txt"},"error":{"message":"permission check failed for command"}}}}"#);
        assert_eq!(parser.denied, [("command(echo hi > a.txt)".to_owned(), "echo hi > a.txt".to_owned())]);
        assert!(parser.parse(line).iter().all(|e| e.name != "permission_denied"));
        // No failed step was reported: the command that never finished is the refused one.
        let mut parser = Parser { permission: Permission::Write, ..Default::default() };
        parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE","tool_info":{"parameters":{"CommandLine":"echo hi > b.txt"}}}}"#);
        assert!(parser.parse(line).iter().all(|e| e.name != "permission_denied"));
        assert_eq!(parser.denied, [("command(echo hi > b.txt)".to_owned(), "echo hi > b.txt".to_owned())]);
        // An MCP tool the mode refused is asked about the same way.
        let mut parser = Parser { permission: Permission::Write, ..Default::default() };
        parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE","tool_info":{"parameters":{"ServerName":"jira","ToolName":"search"}}}}"#);
        parser.parse(line);
        assert_eq!(parser.denied, [("mcp(jira/search)".to_owned(), "MCP jira: search".to_owned())]);
        assert!(run(&[r#"{"event":"result","result":{"status":"SUCCESS","response":"ok","denied_actions":[]}}"#]).iter().all(|(n, _)| n != "permission_denied"));
    }

    #[test]
    fn read_denies_file_writes_and_no_command_is_pre_allowed() {
        let rules = |permission| permission_rules(permission, &["command(npm test)".to_owned()])["permissions"].clone();
        assert_eq!(rules(Permission::Read)["deny"], json!(["write_file(*)"]));
        assert_eq!(rules(Permission::Write)["deny"], json!([]));
        let allow = |permission| rules(permission)["allow"].to_string();
        assert_eq!(allow(Permission::Read), r#"["mcp(codegraph/codegraph_explore)","command(npm test)"]"#);
        assert!(allow(Permission::Full).contains("mcp(codegraph/codegraph_explore)"));
        // A write refused by the Read rule offers the level above.
        let mut parser = Parser { permission: Permission::Read, ..Default::default() };
        parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ERROR","tool_info":{"parameters":{"Path":"C:/a.txt"},"error":{"message":"permission check failed for write_file"}}}}"#);
        let events = parser.parse(r#"{"event":"result","result":{"status":"SUCCESS","response":"no"}}"#);
        assert!(events.iter().any(|e| e.name == "permission_denied" && e.data["need"] == "write"));
    }

    #[test]
    fn the_budget_counts_growth_since_the_last_compaction() {
        let id = "test-conversation-size";
        note_input(id, 100_000);
        assert_eq!(super::super::codex::compact::needs_compaction(id, 150_000), None);
        note_input(id, 200_000);
        assert_eq!(super::super::codex::compact::needs_compaction(id, 150_000), Some(200_000));
        // After a compaction the running input stays high, but only the growth after it counts.
        sizes().lock().unwrap().insert(id.to_owned(), (210_000, 210_000));
        super::super::codex::compact::forget(id);
        note_input(id, 240_000);
        assert_eq!(super::super::codex::compact::needs_compaction(id, 150_000), None);
        assert_eq!(super::super::codex::compact::needs_compaction(id, 20_000), Some(30_000));
        super::super::codex::compact::forget(id);
    }

    #[test]
    fn permissions_map_to_agy_modes() {
        assert_eq!(mode(Permission::Read), "accept-edits");
        assert_eq!(mode(Permission::Write), "accept-edits");
        assert_eq!(mode(Permission::Full), "auto");
    }
}
