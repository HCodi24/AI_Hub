use super::{cli_support, Provider, ProviderEvent, ProviderRequest};
use crate::chat::{Effort, Permission};
use anyhow::{bail, Result};
use async_trait::async_trait;
use serde_json::Value;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct CodexCliProvider;

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
        let mut args = vec![OsString::from("exec")];
        if request.session_id.is_some() {
            args.push(OsString::from("resume"));
        }
        // Both exec and exec resume accept configuration overrides. Keep options
        // before the optional session and prompt positional arguments.
        let sandbox_mode = match request.permission {
            Permission::Read => "read-only",
            Permission::Write => "workspace-write",
        };
        args.extend([
            OsString::from("--json"),
            OsString::from("--skip-git-repo-check"),
            OsString::from("-c"),
            OsString::from(format!("sandbox_mode=\"{sandbox_mode}\"")),
        ]);
        if !request.model.is_empty() {
            args.extend([OsString::from("-m"), OsString::from(&request.model)]);
        }
        if request.effort != Effort::Default {
            args.extend([
                OsString::from("-c"),
                OsString::from(format!(
                    "model_reasoning_effort=\"{}\"",
                    match request.effort {
                        Effort::Low => "low",
                        Effort::Medium => "medium",
                        Effort::High => "high",
                        Effort::Xhigh => "xhigh",
                        Effort::Max => "max",
                        Effort::Default => unreachable!(),
                    }
                )),
            ]);
        }
        if let Some(session_id) = request.session_id.as_deref() {
            args.push(OsString::from(session_id));
        }
        args.push(OsString::from("-"));
        let prompt = cli_support::compose_prompt(&request.messages, request.session_id.is_some());
        cli_support::run_jsonl(
            cli_support::JsonlCommand {
                cli_name: "Codex CLI",
                executable_name: "codex",
                missing_message: "Chưa cài Codex CLI: npm i -g @openai/codex",
                configured_executable: request.provider.executable.clone(),
                folder: &request.folder,
                args,
                prompt,
            },
            events,
            cancel,
            parse_codex_line,
        )
        .await
    }
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
