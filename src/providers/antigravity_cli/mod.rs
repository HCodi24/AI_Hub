//! Antigravity CLI (`agy`): one headless turn per message, JSON in and out (`--input-format/--output-format stream-json`).
use super::{cli_support, Provider, ProviderEvent, ProviderRequest};
use crate::chat::Permission;
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::json;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct AntigravityCliProvider;

mod compact;
mod home;
mod parser;
use crate::features::providers::permission::antigravity as permission;
#[cfg(test)]
mod tests;

pub(crate) use compact::compact_session;
use compact::note_input;
pub use home::{clean_home, sync_login_mode};
use home::write_mcp_config;
pub(crate) use home::{session_prefix, SIGNED_OUT_FILE};
use parser::Parser;
use permission::{mode, permission_rules};

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
