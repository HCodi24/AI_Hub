use super::{request, run::Run};
use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard, RunningChat},
    chat::{self, ChatMessage, ChatRequest},
    mutis::{self, Event},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde_json::json;
use std::{convert::Infallible, time::Instant};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// What the chat avatar is chosen by: the source kind, except DeepSeek models (by model, source id or name), which get their own.
fn avatar_source(provider: &crate::config::ProviderConfig, model: &str) -> String {
    let deepseek = [model, provider.id.as_str(), provider.name.as_str()]
        .iter()
        .any(|text| text.to_ascii_lowercase().contains("deepseek"));
    if deepseek && !provider.kind.ends_with("_cli") {
        "deepseek".to_owned()
    } else {
        provider.kind.clone()
    }
}

/// First user message of the conversation gets the fixed rules in front of it.
fn add_first_request_rules(request: &mut ProviderRequest) {
    // The DeepSeek agent has no MCP, so the CodeGraph rule does not apply to it.
    if request.provider.kind == "deepseek_agent"
        || request.messages.iter().filter(|m| m.role == "user").count() != 1
    {
        return;
    }
    let indexed = std::path::Path::new(&request.folder)
        .join(".codegraph")
        .is_dir();
    if let Some(message) = request.messages.iter_mut().find(|m| m.role == "user") {
        message.content = format!(
            "{}

{}",
            crate::core::rule::first_request_rules(indexed),
            message.content
        );
    }
}

fn add_question_rules(request: &mut ProviderRequest) {
    if let Some(message) = request.messages.iter_mut().rev().find(|m| m.role == "user") {
        message.content = format!("{}\n\n{}", crate::core::rule::chat_questions::RULE, message.content);
    }
}

/// Cancels the run when the browser drops the stream.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(crate) async fn chat_handler(
    State(state): State<AppState>,
    Json(request): Json<ChatRequest>,
) -> ApiResult<Response> {
    if !chat::valid_id(&request.chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    let existing = state.chats.get(&request.chat_id).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể đọc lịch sử chat",
        )
    })?;
    let dev_chat = request.dev_chat || existing.as_ref().is_some_and(|chat| chat.dev_chat);
    let folder = request::resolve_folder(&state, &request, dev_chat)?;
    let plain_chat = !dev_chat && request.folder.trim().is_empty();
    let effort = request::check_request(&request)?;
    let mut provider_config = {
        let config = state.config.read().await;
        // Old chats may still point at the removed "DeepSeek làm việc" source: resolve it to the Deploy source.
        crate::config::resolve_provider_id(&config.providers, &request.provider_id)
            .cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?
    };
    // File tools = agent mode of the SAME source; Full also grants API-key sources run_command.
    let file_tools = if plain_chat { chat::FileTools::None } else { request.file_tools };
    let agent = chat::agent_permission(&provider_config, file_tools, request.permission)
        .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
    if provider_config.kind == "anthropic" {
        // Anthropic keeps its provider kind; the request-local flag tells it to enter the file-tool loop.
        provider_config.agent_capable = agent.is_some();
    } else if agent.is_some() {
        provider_config.kind = "deepseek_agent".into();
    } else if provider_config.kind == "openai_compat"
        && request
            .messages
            .last()
            .is_none_or(|message| message.attachments.is_empty())
        && state
            .config
            .read()
            .await
            .mcp_servers
            .iter()
            .any(|server| server.enabled)
    {
        // MCP servers enabled in the app: this key-based source runs the tool loop (MCP tools only, no file tools).
        // A message with images stays a plain chat: the tool loop sends text only.
        provider_config.kind = "mcp_agent".into();
    }
    let mutis_config = state.config.read().await.mutis.clone();
    // MutiS High + Codex: the app runs the DeepSeek worker for Codex (see providers::delegate).
    let role_effort_name = request.effort.as_deref().unwrap_or("max");
    let delegate_worker = if provider_config.kind == "codex_cli" && mutis_config.enabled && request.plan_id.is_none() && !plain_chat
        && crate::config::MutisConfig::level_for(role_effort_name) == "high" {
        let config = state.config.read().await;
        // High roles may be unset (old configs): fall back to the Max small worker.
        let high_small = &mutis_config.roles_for(role_effort_name).small;
        let role = if high_small.provider_id.trim().is_empty() { &mutis_config.roles.small } else { high_small };
        crate::config::resolve_provider_id(&config.providers, &role.provider_id)
            .filter(|p| !p.api_key.trim().is_empty())
            .map(|p| (p.clone(), if role.model.trim().is_empty() { p.models.first().cloned().unwrap_or_default() } else { role.model.clone() }))
            .filter(|(_, model)| !model.is_empty())
    } else {
        None
    };
    let provider: Box<dyn providers::Provider> = match &delegate_worker {
        Some((worker, model)) => Box::new(providers::delegate::DelegateProvider { worker: worker.clone(), model: model.clone() }),
        None => providers::for_kind(&provider_config.kind)
            .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Loại nhà cung cấp không hợp lệ"))?,
    };
    let switched_source = existing
        .as_ref()
        .is_some_and(|r| r.provider_id != request.provider_id);
    let mut record = existing.unwrap_or_else(|| chat::new_record(&request));
    if switched_source {
        record.switch_source(&request.provider_id);
    }
    if provider_config.kind == "codex_cli" {
        let settings = state.config.read().await.codex.clone();
        let prefix = providers::codex::session_prefix(&settings, &provider_config, &folder);
        if !record.session_id.as_deref().is_some_and(|id| id.starts_with(&prefix)) {
            record.session_id = None;
        }
    }
    record.provider_id = request.provider_id.clone();
    record.model = request.model.clone();
    record.folder = if plain_chat { String::new() } else { folder.clone() };
    record.dev_chat = dev_chat;
    record.permission = request.permission;
    record.file_tools = file_tools;
    record.effort = effort;
    record.messages = request
        .messages
        .iter()
        .map(|message| {
            let mut saved = message.clone();
            if saved.role == "assistant" {
                saved.content = crate::features::memory::strip_blocks(&saved.content).0;
            }
            saved.attachments = crate::api::attachments::resolve_message_attachments(
                &state,
                &request.chat_id,
                &message.attachments,
            )?;
            Ok(saved)
        })
        .collect::<ApiResult<Vec<_>>>()?;
    if dev_chat {
        let preamble = crate::api::dev::dev_chat_preamble(&folder);
        if let Some(first) = record
            .messages
            .iter_mut()
            .find(|message| message.role == "user")
        {
            if !first.content.starts_with(&preamble) {
                first.content = format!("{preamble}\n\n{}", first.content);
            }
        }
    }
    record.messages.push(ChatMessage {
        debate_id: None,
        plan_id: None,
        role: "assistant".to_owned(),
        content: String::new(),
        reasoning: String::new(),
        logs: Vec::new(),
        diff: String::new(),
        files: Vec::new(),
        provider: avatar_source(&provider_config, &request.model),
        error: None,
        attachments: Vec::new(),
    });
    chat::save_record(&state.chats, &mut record).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể lưu lịch sử chat",
        )
    })?;
    let cancel = CancellationToken::new();
    let request_id = Uuid::new_v4();
    {
        let mut running = state.running.lock().await;
        if let Some(previous) = running.insert(
            request.chat_id.clone(),
            RunningChat {
                request_id,
                cancel: cancel.clone(),
            },
        ) {
            previous.cancel.cancel();
        }
    }
    let (sse_sender, sse_receiver) = mpsc::channel::<axum::response::sse::Event>(32);
    let (provider_sender, provider_receiver) = mpsc::channel::<ProviderEvent>(32);
    let provider_cancel = cancel.clone();
    let mut memory_config = state.config.read().await.memory.clone();
    // Plan task workers neither read nor write the memory: the planner already got it, the main chat AI writes it.
    if request.plan_id.is_some() || plain_chat {
        memory_config.enabled = false;
    }
    let memory_context =
        crate::features::memory::memory_context(&state.config_path, &folder, &memory_config);
    let files_before = if plain_chat { Default::default() } else {
        crate::features::changes::snapshot(std::path::Path::new(&folder)).await
    };
    let mut provider_request = ProviderRequest {
        provider: provider_config,
        model: request.model,
        folder,
        permission: agent.unwrap_or(request.permission),
        effort,
        messages: record
            .messages
            .iter()
            .take(record.messages.len().saturating_sub(1))
            // Failed/empty assistant turns would be rejected by chat APIs.
            .filter(|message| message.role != "assistant" || !message.content.trim().is_empty())
            .cloned()
            .map(|mut message| {
                if message.plan_id.is_some() || message.debate_id.is_some() {
                    message.plan_id = None;
                    message.debate_id = None;
                    message.logs.clear();
                    message.reasoning.clear();
                    message.diff.clear();
                    message.attachments.clear();
                }
                message
            })
            .collect(),
        session_id: record.session_id.clone(),
        mutis: mutis_config.clone(),
        mcp: state
            .config
            .read()
            .await
            .mcp_servers
            .iter()
            .filter(|server| server.enabled)
            .cloned()
            .collect(),
        write_scope: Vec::new(),
    };
    if memory_config.enabled {
        crate::features::memory::inject(&mut provider_request, &memory_context);
    }
    // Plan workers run unattended; interactive chat supports user decision cards on every turn.
    if request.plan_id.is_none() {
        add_question_rules(&mut provider_request);
        if !plain_chat {
            let rule = crate::core::rule::mutis_high_delegation::for_chat(
                mutis_config.enabled,
                crate::config::MutisConfig::level_for(request.effort.as_deref().unwrap_or("max")),
                &provider_request.provider.kind,
                provider_request.mcp.iter().any(|server| server.name.contains("deepseek")),
                delegate_worker.is_some(),
            );
            if !rule.is_empty() {
                if let Some(message) = provider_request.messages.iter_mut().rev().find(|m| m.role == "user") {
                    message.content = format!("{rule}\n\n{}", message.content);
                }
            }
        }
    }
    // Plan task workers cannot stop to ask the user, so they skip the first-request rules.
    if request.plan_id.is_none() && !plain_chat {
        add_first_request_rules(&mut provider_request);
    }
    let event_provider_id = provider_request.provider.id.clone();
    let provider_name = provider_request.provider.name.clone();
    let provider_kind = avatar_source(&provider_request.provider, &provider_request.model);
    let status_model = provider_request.model.clone();
    let status_effort = format!("{:?}", provider_request.effort).to_lowercase();
    let role_effort = request.effort.as_deref().unwrap_or("max");
    let status_role = (mutis_config.enabled || request.plan_id.is_some())
        .then(|| {
            ["lead", "big", "small"]
                .into_iter()
                .find(|role| match *role {
                    "lead" => mutis_config.roles_for(role_effort).lead.provider_id == event_provider_id,
                    "big" => mutis_config.roles_for(role_effort).big.provider_id == event_provider_id,
                    _ => mutis_config.roles_for(role_effort).small.provider_id == event_provider_id,
                })
                .map(str::to_owned)
        })
        .flatten();
    let idle_timeout_minutes = state.config.read().await.chat_idle_timeout_minutes;
    let provider_events = provider_sender.clone();
    tokio::spawn(async move {
        if let Err(error) = provider
            .run(provider_request, provider_sender, provider_cancel)
            .await
        {
            let _ = provider_events
                .send(ProviderEvent::text("error", error.to_string()))
                .await;
        }
    });
    let task_state = state.clone();
    let chat_id = request.chat_id;
    let plan_id = request.plan_id;
    let task_id = request.task_id;
    let task_kind = request.task_kind;
    let event_logging_enabled = mutis_config.effective(mutis_config.event_log);
    let log_enabled = event_logging_enabled && plan_id.is_some() && task_id.is_some();
    let started = Instant::now();
    let started_at_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let _ = sse_sender
        .send(chat::sse_event(
            "status",
            &json!({
                "provider_name": provider_name,
                "provider_kind": provider_kind,
                "model": status_model,
                "effort": status_effort,
                "role": status_role,
                "started_at_ms": started_at_ms,
            }),
        ))
        .await;
    if log_enabled {
        let _ = state.event_log.append(&Event {
            ts: mutis::now(),
            plan_id: plan_id.clone(),
            task_id: task_id.clone(),
            provider_id: event_provider_id.clone(),
            kind: "task_start".into(),
            task_kind: task_kind.clone(),
            ok: true,
            duration_ms: None,
            tokens: None,
            detail: None,
        });
    }
    let task_cancel = cancel.clone();
    let job_guard = JobGuard::new(state.active_jobs.clone());
    tokio::spawn(
        Run {
            job_guard,
            record,
            task_state,
            chat_id,
            request_id,
            plan_id,
            task_id,
            task_kind,
            event_provider_id,
            event_logging_enabled,
            log_enabled,
            started,
            task_cancel,
            sse_sender,
            provider_receiver,
            idle_timeout_minutes,
            memory_config,
            files_before,
        }
        .run(),
    );
    let stream_cancel = cancel.clone();
    let event_stream = async_stream::stream! {
        let _guard = CancelOnDrop(stream_cancel);
        let mut receiver = ReceiverStream::new(sse_receiver);
        while let Some(event) = receiver.next().await {
            yield Ok::<_, Infallible>(event);
        }
    };
    Ok(Sse::new(event_stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(messages: serde_json::Value) -> ProviderRequest {
        ProviderRequest {
            provider: Default::default(),
            model: "m".into(),
            folder: "f".into(),
            permission: Default::default(),
            effort: Default::default(),
            messages: serde_json::from_value(messages).unwrap(),
            session_id: None,
            mutis: Default::default(),
            mcp: Vec::new(),
            write_scope: Vec::new(),
        }
    }

    #[test]
    fn deepseek_models_get_their_own_avatar_label() {
        let source = |id: &str, kind: &str| crate::config::ProviderConfig { id: id.into(), kind: kind.into(), ..Default::default() };
        assert_eq!(avatar_source(&source("openai", "openai_compat"), "deepseek-ai/deepseek-v4"), "deepseek");
        assert_eq!(avatar_source(&source("deploy-deepseek", "mcp_agent"), "x"), "deepseek");
        assert_eq!(avatar_source(&source("openai", "openai_compat"), "gpt-5.1"), "openai_compat");
        assert_eq!(avatar_source(&source("codex-cli", "codex_cli"), "gpt-6-luna"), "codex_cli");
    }

    #[test]
    fn rules_go_only_on_the_first_user_message() {
        let mut first = request(json!([{"role":"user","content":"làm đi"}]));
        add_first_request_rules(&mut first);
        assert!(first.messages[0].content.contains(".codegraph"));
        assert!(first.messages[0].content.ends_with("làm đi"));
        // The app states the index status itself, so the AI does not spend a command (and a model call) on it.
        assert!(first.messages[0]
            .content
            .contains("DOES NOT HAVE a .codegraph folder"));
        let mut later = request(json!([
            {"role":"user","content":"a"},{"role":"assistant","content":"b"},{"role":"user","content":"c"}
        ]));
        add_first_request_rules(&mut later);
        assert!(later
            .messages
            .iter()
            .all(|m| !m.content.contains(".codegraph")));
    }

    #[test]
    fn question_rules_reach_only_the_current_user_message() {
        let mut turn = request(json!([
            {"role":"user","content":"first"},
            {"role":"assistant","content":"reply"},
            {"role":"user","content":"follow up"}
        ]));
        add_question_rules(&mut turn);
        assert_eq!(turn.messages[0].content, "first");
        assert_eq!(turn.messages[1].content, "reply");
        assert!(turn.messages[2].content.starts_with(crate::core::rule::chat_questions::RULE));
        assert!(turn.messages[2].content.ends_with("follow up"));
    }
}
