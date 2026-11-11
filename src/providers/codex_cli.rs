use super::{cli_support, Provider, ProviderEvent, ProviderRequest};
use crate::chat::{Effort, Permission};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct CodexCliProvider;

pub const EARLY_STOP_PHRASES: &[&str] = &[
    "incomplete",
    "not finished",
    "chưa xong",
    "chưa hoàn thành",
    "remaining work",
    "i stopped",
];

pub fn indicates_early_stop(message: &str) -> bool {
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let lowered = normalized
        .trim()
        .split("\n\n")
        .last()
        .unwrap_or_default()
        .to_lowercase();
    EARLY_STOP_PHRASES
        .iter()
        .any(|phrase| lowered.contains(phrase))
}

#[async_trait]
impl Provider for CodexCliProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if !cli_support::valid_model(&request.model) {
            bail!("Tên model chỉ được chứa chữ, số, ., _, :, / và -");
        }
        if let Some(session_id) = request.session_id.as_deref() {
            if !cli_support::safe_session_id(session_id) {
                bail!("Codex session id không hợp lệ");
            }
        }
        let image_dir = super::latest_chat_attachment_dir(&request.messages);
        let image_paths = image_dir
            .as_deref()
            .map(|dir| super::latest_image_paths(&request.messages, dir))
            .transpose()?
            .unwrap_or_default();
        let mut args = codex_args_with_images(
            request.session_id.as_deref(),
            &request.model,
            request.effort,
            request.permission,
            &image_paths,
        );
        let mut prompt =
            cli_support::compose_prompt(&request.messages, request.session_id.is_some());
        let mut thread_id = request.session_id.clone();
        let mut final_message = String::new();
        let mut turn_completed = false;
        let mut resume_count = 0;
        loop {
            let parse_message = &mut final_message;
            let parse_thread = &mut thread_id;
            let parse_completed = &mut turn_completed;
            let result = cli_support::run_jsonl(
                cli_support::JsonlCommand {
                    cli_name: "Codex CLI",
                    executable_name: "codex",
                    missing_message: "Chưa cài Codex CLI: npm i -g @openai/codex",
                    configured_executable: request.provider.executable.clone(),
                    folder: &request.folder,
                    args: args.clone(),
                    prompt,
                    watchdog_minutes: request
                        .mutis
                        .effective(request.mutis.codex_watchdog)
                        .then_some(request.mutis.watchdog_minutes),
                },
                events.clone(),
                cancel.clone(),
                move |line| {
                    if let Ok(value) = serde_json::from_str::<Value>(line) {
                        if value["type"] == "thread.started" {
                            *parse_thread = value["thread_id"].as_str().map(str::to_owned);
                        }
                        if value["type"] == "turn.completed" {
                            *parse_completed = true;
                        }
                        let item = &value["item"];
                        if value["type"] == "item.completed"
                            && matches!(
                                item["type"].as_str(),
                                Some("agent_message" | "assistant_message")
                            )
                        {
                            *parse_message = item["text"]
                                .as_str()
                                .or_else(|| item["content"].as_str())
                                .unwrap_or_default()
                                .to_owned();
                        }
                    }
                    parse_codex_line(line)
                },
            )
            .await;
            result?;
            let max = request.mutis.resume_max.min(3);
            if !request.mutis.effective(request.mutis.codex_auto_resume)
                || !turn_completed
                || resume_count >= max
                || !thread_id
                    .as_deref()
                    .is_some_and(cli_support::safe_session_id)
                || !indicates_early_stop(&final_message)
                || cancel.is_cancelled()
            {
                break;
            }
            resume_count += 1;
            let _ = events
                .send(ProviderEvent::text(
                    "log",
                    format!("Tự tiếp tục lần {resume_count}/{max}"),
                ))
                .await;
            final_message.clear();
            turn_completed = false;
            let id = thread_id.clone().unwrap_or_default();
            args = codex_args_with_images(
                Some(&id),
                &request.model,
                request.effort,
                request.permission,
                &image_paths,
            );
            prompt =
                "Tiếp tục và hoàn thành phần còn lại của nhiệm vụ. Báo cáo rõ những gì chưa xong."
                    .to_owned();
        }
        Ok(())
    }
}

#[cfg(test)]
fn codex_args(
    session_id: Option<&str>,
    model: &str,
    effort: Effort,
    permission: Permission,
) -> Vec<OsString> {
    codex_args_with_images(session_id, model, effort, permission, &[])
}

pub fn codex_args_with_images(
    session_id: Option<&str>,
    model: &str,
    effort: Effort,
    permission: Permission,
    images: &[std::path::PathBuf],
) -> Vec<OsString> {
    let mut args = vec![OsString::from("exec")];
    if session_id.is_some() {
        args.push(OsString::from("resume"));
    }
    let sandbox_mode = match permission {
        Permission::Read => "read-only",
        // Codex already permits commands within the workspace-write sandbox.
        Permission::Write | Permission::Full => "workspace-write",
    };
    args.extend([
        OsString::from("--json"),
        OsString::from("--skip-git-repo-check"),
        OsString::from("-c"),
        OsString::from(format!("sandbox_mode=\"{sandbox_mode}\"")),
    ]);
    if !model.is_empty() {
        args.extend([OsString::from("-m"), OsString::from(model)]);
    }
    if effort != Effort::Default {
        let value = match effort {
            Effort::Low => "low",
            Effort::Medium => "medium",
            Effort::High => "high",
            Effort::Xhigh => "xhigh",
            Effort::Max => "max",
            Effort::Default => unreachable!(),
        };
        args.extend([
            OsString::from("-c"),
            OsString::from(format!("model_reasoning_effort=\"{value}\"")),
        ]);
    }
    // Use the single-token `--image=<path>` form: `-i <FILE>...` takes MANY values in clap, so it would
    // swallow the following session id and the `-` (read prompt from stdin) marker.
    for image in images {
        let mut arg = OsString::from("--image=");
        arg.push(image.as_os_str());
        args.push(arg);
    }
    if let Some(id) = session_id {
        args.push(OsString::from(id));
    }
    args.push(OsString::from("-"));
    args
}

fn parse_codex_line(line: &str) -> Vec<ProviderEvent> {
    if line.trim().is_empty() {
        return Vec::new();
    }
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return vec![ProviderEvent::text(
            "log",
            format!("Codex output: {}", cli_support::one_line(line, 240)),
        )];
    };
    let event_type = value["type"].as_str().unwrap_or("unknown");
    let mut events = Vec::new();
    if event_type == "thread.started" {
        if let Some(session_id) = value["thread_id"]
            .as_str()
            .or_else(|| value["session_id"].as_str())
            .filter(|id| cli_support::safe_session_id(id))
        {
            events.push(ProviderEvent {
                name: "session",
                data: serde_json::json!({ "session_id": session_id }),
            });
        }
    }

    match event_type {
        "thread.started" | "turn.started" | "turn.interrupted" => {}
        "turn.completed" => {
            if let Some(usage) = cli_support::token_usage_log(&value["usage"], None) {
                events.push(ProviderEvent::text("log", usage));
            }
        }
        "error" | "turn.failed" => {
            let error = value["error"]["message"]
                .as_str()
                .or_else(|| value["message"].as_str())
                .map(str::to_owned)
                .unwrap_or_else(|| value["error"].to_string());
            events.push(ProviderEvent::text("error", error));
        }
        "item.started" | "item.updated" | "item.completed" | "item.delta" => {
            let item = &value["item"];
            let item_type = item["type"].as_str().unwrap_or("");
            let is_delta = event_type == "item.delta";
            let delta_text = value["delta"]
                .as_str()
                .or_else(|| value["delta"]["text"].as_str());
            match item_type {
                "agent_message" | "assistant_message" => {
                    let text = if is_delta {
                        delta_text
                    } else if event_type == "item.completed" {
                        item["text"].as_str().or_else(|| item["content"].as_str())
                    } else {
                        None
                    };
                    if let Some(text) = text.filter(|text| !text.is_empty()) {
                        events.push(ProviderEvent::text("delta", text.to_owned()));
                    }
                }
                "reasoning" => {
                    let text = if is_delta {
                        delta_text
                    } else if event_type == "item.completed" {
                        item["text"].as_str().or_else(|| item["content"].as_str())
                    } else {
                        None
                    };
                    if let Some(text) = text.filter(|text| !text.is_empty()) {
                        events.push(ProviderEvent::text("reasoning", text.to_owned()));
                    }
                }
                "command_execution" | "command" => {
                    if event_type == "item.started" || event_type == "item.completed" {
                        let command = item["command"]
                            .as_str()
                            .map(str::to_owned)
                            .or_else(|| {
                                item["command"].as_array().map(|parts| {
                                    parts
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .collect::<Vec<_>>()
                                        .join(" ")
                                })
                            })
                            .unwrap_or_else(|| "command".to_owned());
                        let prefix = if event_type == "item.started" {
                            "Command"
                        } else {
                            "Command finished"
                        };
                        events.push(ProviderEvent::text(
                            "log",
                            format!("{prefix}: {}", cli_support::one_line(&command, 220)),
                        ));
                    }
                }
                "file_change" | "file_changes" => {
                    if event_type == "item.started" || event_type == "item.completed" {
                        let details = file_change_summary(item);
                        events.push(ProviderEvent::text("log", details));
                    }
                }
                "mcp_tool_call" | "tool_call" | "web_search" | "tool_use" => {
                    if event_type == "item.started" || event_type == "item.completed" {
                        let tool = item["name"]
                            .as_str()
                            .or_else(|| item["tool_name"].as_str())
                            .unwrap_or(item_type);
                        events.push(ProviderEvent::text(
                            "log",
                            format!("Tool: {}", cli_support::one_line(tool, 220)),
                        ));
                    }
                }
                "error" => {
                    let message = item["message"]
                        .as_str()
                        .unwrap_or("Lỗi không nghiêm trọng từ Codex");
                    events.push(ProviderEvent::text(
                        "log",
                        format!("Cảnh báo: {}", cli_support::one_line(message, 220)),
                    ));
                }
                "" => {
                    if is_delta {
                        if let Some(text) = delta_text.filter(|text| !text.is_empty()) {
                            events.push(ProviderEvent::text("delta", text.to_owned()));
                        }
                    } else if event_type == "item.completed" {
                        events.push(ProviderEvent::text(
                            "log",
                            format!(
                                "Codex item: {}",
                                cli_support::one_line(&item.to_string(), 220)
                            ),
                        ));
                    }
                }
                other => {
                    if event_type == "item.started" || event_type == "item.completed" {
                        events.push(ProviderEvent::text(
                            "log",
                            format!(
                                "Codex item {other}: {}",
                                cli_support::one_line(&item.to_string(), 220)
                            ),
                        ));
                    }
                }
            }
        }
        _ => events.push(ProviderEvent::text(
            "log",
            format!(
                "Codex event {event_type}: {}",
                cli_support::one_line(&value.to_string(), 220)
            ),
        )),
    }
    events
}

fn file_change_summary(item: &Value) -> String {
    let mut paths = Vec::new();
    if let Some(changes) = item["changes"].as_array() {
        for change in changes {
            if let Some(path) = change["path"].as_str().or_else(|| change["file"].as_str()) {
                paths.push(path.to_owned());
            }
        }
    }
    if paths.is_empty() {
        if let Some(path) = item["path"].as_str() {
            paths.push(path.to_owned());
        }
    }
    if paths.is_empty() {
        format!(
            "File change: {}",
            cli_support::one_line(&item.to_string(), 220)
        )
    } else {
        format!(
            "File change: {}",
            cli_support::one_line(&paths.join(", "), 220)
        )
    }
}

#[cfg(test)]
mod resume_tests {
    use super::{codex_args, indicates_early_stop, EARLY_STOP_PHRASES};
    use crate::chat::{Effort, Permission};

    #[test]
    fn full_uses_workspace_sandbox_for_new_and_resumed_sessions() {
        for session in [None, Some("safe-session")] {
            let full = codex_args(session, "model", Effort::Default, Permission::Full);
            let write = codex_args(session, "model", Effort::Default, Permission::Write);
            assert_eq!(full, write);
            assert!(full
                .iter()
                .any(|arg| arg == "sandbox_mode=\"workspace-write\""));
        }
    }
    #[test]
    fn round_one_and_two_resume_args_are_fresh_and_valid() {
        let first = codex_args(
            Some("thread-123"),
            "gpt-6-luna",
            Effort::High,
            Permission::Write,
        );
        let second = codex_args(
            Some("thread-123"),
            "gpt-6-luna",
            Effort::High,
            Permission::Write,
        );
        assert_eq!(first, second);
        let args = first
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(&args[..2], ["exec", "resume"]);
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == "thread-123")
                .count(),
            1
        );
        assert_eq!(args.last().map(String::as_str), Some("-"));
    }
    #[test]
    fn checks_only_last_paragraph_for_early_stop() {
        assert!(!indicates_early_stop(
            "The quoted word incomplete is not applicable.\n\nEverything is complete."
        ));
        assert!(!indicates_early_stop(
            "The quoted word incomplete is not applicable.\r\n\r\nEverything is complete."
        ));
        assert!(indicates_early_stop(
            "Earlier was fine.\n\nStill incomplete."
        ));
    }
    #[test]
    fn matches_each_early_stop_phrase_without_case_sensitivity() {
        for phrase in EARLY_STOP_PHRASES {
            assert!(indicates_early_stop(&phrase.to_uppercase()));
        }
        assert!(!indicates_early_stop("Task completed successfully."));
    }

    #[test]
    fn codex_image_paths_are_fenced_to_the_chat_attachment_directory() {
        use crate::chat::{ChatAttachment, ChatMessage};
        use std::fs;
        let root = std::env::temp_dir().join(format!("codex-image-{}", uuid::Uuid::new_v4()));
        let inside = root.join("chat");
        fs::create_dir_all(&inside).unwrap();
        let image = inside.join("image.png");
        fs::write(&image, b"image").unwrap();
        let outside = root.join("other.png");
        fs::write(&outside, b"image").unwrap();
        let message = ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "user".into(),
            content: String::new(),
            attachments: vec![ChatAttachment {
                id: "id".into(),
                mime: "image/png".into(),
                path: outside.to_string_lossy().into_owned(),
            }],
            reasoning: String::new(),
            logs: vec![],
            diff: String::new(),
            files: Vec::new(),
            error: None,
        };
        assert!(super::super::latest_image_paths(&[message], &inside).is_err());
        let args = super::codex_args_with_images(
            None,
            "model",
            Effort::Default,
            Permission::Read,
            std::slice::from_ref(&image),
        );
        let values = args
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let expected = format!("--image={}", image.to_string_lossy());
        assert!(values.contains(&expected));
        // The stdin marker must stay the last, separate argument (not swallowed by the image option).
        assert_eq!(values.last().map(String::as_str), Some("-"));
        let _ = fs::remove_dir_all(root);
    }
}
