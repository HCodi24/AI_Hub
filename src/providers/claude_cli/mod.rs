use super::{claude::environment, cli_support, Effort, Provider, ProviderEvent, ProviderRequest};
use crate::config::{ClaudeConfig, ProviderConfig};
use anyhow::{bail, Result};
use async_trait::async_trait;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod compact;
mod parser;
mod permission;

pub(crate) use compact::compact_session;
use parser::ClaudeParser;
use permission::{interaction, permission_args};

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
        let settings = environment::settings()?;
        let home = environment::home(&config_path, &request.provider)?;
        let prefix = environment::session_prefix(&settings, &request.provider, &request.folder);
        prepare_session(&mut request, &config_path, &prefix)?;
        maybe_compact(&request, &home, &config_path, &events, &cancel).await;
        let mut args = build_args(&request, &home, &settings)?;
        let mut prompt = cli_support::compose_prompt_for(
            &request.messages,
            request.session_id.is_some(),
            "claude_cli",
        );
        attach_images(&request, &mut args, &mut prompt)?;
        run_turn(&request, home, &prefix, args, prompt, events, cancel).await
    }
}

fn prepare_session(request: &mut ProviderRequest, config_path: &Path, prefix: &str) -> Result<()> {
    let native = request
        .session_id
        .as_deref()
        .and_then(|id| id.strip_prefix(prefix))
        .map(str::to_owned);
    if request.session_id.is_some() && native.is_none() {
        let config: crate::config::Config = serde_json::from_slice(&std::fs::read(config_path)?)?;
        if let Some(message) = request.messages.last_mut().filter(|message| {
            message.content.contains(crate::features::memory::INSTRUCTION)
        }) {
            let memory = crate::features::memory::memory_context(
                config_path,
                &request.folder,
                &config.memory,
            );
            message.content = format!("{memory}\n\n{}", message.content);
        }
    }
    request.session_id = native;
    Ok(())
}

async fn maybe_compact(
    request: &ProviderRequest,
    home: &Path,
    config_path: &Path,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) {
    // Past the token budget the thread is compacted in place (`/compact`), as Codex threads are, instead of being dropped.
    if let Some(id) = request.session_id.clone() {
        if let Some(size) = super::codex::compact::needs_compaction(&id, super::codex::compact::thread_token_budget(config_path)) {
            let _ = events.send(ProviderEvent::text("log", format!("Luồng Claude đã đọc {size} token; đang nén ngữ cảnh."))).await;
            let note = match compact_session(&request.provider, home, &request.folder, &id, &request.model, cancel).await {
                Ok(()) => "Đã nén ngữ cảnh luồng Claude.".to_owned(),
                Err(error) => format!("Không nén được luồng Claude, dùng tiếp như cũ: {error}"),
            };
            let _ = events.send(ProviderEvent::text("log", note)).await;
        }
    }
}

fn effort_arg(effort: Effort) -> Option<&'static str> {
    match effort {
        Effort::Default | Effort::None => None,
        Effort::Low | Effort::Minimal => Some("low"),
        Effort::Medium => Some("medium"),
        Effort::High => Some("high"),
        Effort::Xhigh => Some("xhigh"),
        Effort::Max | Effort::Ultra => Some("max"),
    }
}

fn build_args(request: &ProviderRequest, home: &Path, settings: &ClaudeConfig) -> Result<Vec<OsString>> {
    let mut args = vec![
        OsString::from("-p"),
        OsString::from("--input-format"),
        OsString::from("stream-json"),
        OsString::from("--permission-prompt-tool"),
        OsString::from("stdio"),
        OsString::from("--output-format"),
        OsString::from("stream-json"),
        OsString::from("--verbose"),
        OsString::from("--include-partial-messages"),
    ];
    args.extend(environment::launch_args(
        &request.mcp,
        home,
        environment::enabled_hub_mcp(home, &settings.enabled_mcp),
    ));
    let context = super::claude::sources::project_context(
        Path::new(&request.folder),
        settings,
    )?;
    if !context.is_empty() {
        args.extend([OsString::from("--append-system-prompt"), context.into()]);
    }
    args.extend(permission_args(request.permission));
    if !request.model.is_empty() {
        args.extend([OsString::from("--model"), OsString::from(&request.model)]);
    }
    if let Some(effort) = effort_arg(request.effort) {
        args.extend([OsString::from("--effort"), OsString::from(effort)]);
    }
    if let Some(session_id) = request.session_id.as_deref() {
        if !cli_support::safe_session_id(session_id) {
            bail!("Claude session id không hợp lệ");
        }
        args.extend([OsString::from("--resume"), OsString::from(session_id)]);
    }
    Ok(args)
}

fn attach_images(request: &ProviderRequest, args: &mut Vec<OsString>, prompt: &mut String) -> Result<()> {
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
    Ok(())
}

async fn run_turn(
    request: &ProviderRequest,
    home: PathBuf,
    prefix: &str,
    args: Vec<OsString>,
    prompt: String,
    events: mpsc::Sender<ProviderEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    let parser = std::sync::Mutex::new(ClaudeParser {
        permission: request.permission,
        resumed: request.session_id.is_some(),
        ..Default::default()
    });
    let result = cli_support::run_jsonl(
        jsonl_command(
            &request.provider,
            home,
            &request.folder,
            args,
            format!("{}\n", serde_json::json!({"type": "user", "message": {"role": "user", "content": prompt}})),
            Some(interaction(request.permission)),
        ),
        events.clone(),
        cancel.clone(),
        |line| {
            super::limits::record_claude(&request.provider.id, line);
            let mut parsed = parser
                .lock()
                .expect("Claude parser mutex poisoned")
                .parse(line);
            prefix_session_events(&mut parsed, prefix);
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

fn prefix_session_events(events: &mut [ProviderEvent], prefix: &str) {
    for event in events {
        if event.name == "session" {
            if let Some(id) = event.data["session_id"].as_str() {
                event.data = serde_json::json!({"session_id":format!("{prefix}{id}")});
            }
        }
    }
}

pub(super) fn jsonl_command<'a>(
    provider: &ProviderConfig,
    home: PathBuf,
    folder: &'a str,
    args: Vec<OsString>,
    prompt: String,
    interact: Option<cli_support::Interact>,
) -> cli_support::JsonlCommand<'a> {
    cli_support::JsonlCommand {
        config_dir: Some(home),
        cli_name: "Claude Code CLI",
        executable_name: "claude",
        missing_message: "Chưa cài Claude Code CLI: npm i -g @anthropic-ai/claude-code",
        configured_executable: provider.executable.clone(),
        folder,
        args,
        prompt,
        watchdog_minutes: None,
        interact,
    }
}

fn missing_result_event(result_seen: bool, cancelled: bool) -> bool {
    !result_seen && !cancelled
}

#[cfg(test)]
mod tests;
