use super::{request, run::Run, skills};
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
    if provider.is_deepseek(model) {
        "deepseek".to_owned()
    } else {
        provider.kind.clone()
    }
}

/// Kinds of source that can run delegated tasks: an API agent loop (OpenAI-compatible) or a CLI thread (Codex, Claude).
const WORKER_KINDS: [&str; 3] = ["openai_compat", "codex_cli", "claude_cli"];

/// Sources the delegated worker may be, best first: the role's own source, then the agent-capable API sources, then the Codex sources.
/// The caller uses the first one that is active (connected).
fn worker_candidates(
    providers: &[crate::config::ProviderConfig],
    configured_id: &str,
) -> Vec<crate::config::ProviderConfig> {
    let mut found: Vec<_> = crate::config::resolve_provider_id(providers, configured_id)
        .filter(|p| WORKER_KINDS.contains(&p.kind.as_str()))
        .cloned()
        .into_iter()
        .collect();
    for provider in providers {
        let fits = (provider.kind == "openai_compat" && provider.agent_capable)
            || provider.kind == "codex_cli";
        if fits && !found.iter().any(|p| p.id == provider.id) {
            found.push(provider.clone());
        }
    }
    found
}

/// Only the CLI sources get CodeGraph's MCP tool from the app; API sources and the DeepSeek agent cannot call it.
fn uses_codegraph(request: &ProviderRequest) -> bool {
    matches!(request.provider.kind.as_str(), "codex_cli" | "claude_cli" | "antigravity_cli")
}

/// First user message of the conversation gets the fixed rules in front of it. Before that the app builds or syncs
/// the project's CodeGraph index; problems go to the chat's activity log through `events`.
async fn add_first_request_rules(request: &mut ProviderRequest, events: &mpsc::Sender<ProviderEvent>) {
    if !uses_codegraph(request) || request.messages.iter().filter(|m| m.role == "user").count() != 1 {
        return;
    }
    let prepared = crate::features::extensions::prepare(std::path::Path::new(&request.folder)).await;
    for note in prepared.notes {
        let _ = providers::emit_text(events, "log", note).await;
    }
    if let Some(message) = request.messages.iter_mut().find(|m| m.role == "user") {
        message.content = format!(
            "{}\n\n{}",
            crate::core::rule::first_request_rules(prepared.indexed, prepared.synced),
            message.content
        );
    }
}

/// Gives the AI CodeGraph's MCP server (every turn, so a resumed session keeps the tool) once the project has an index.
fn attach_codegraph(request: &mut ProviderRequest) {
    let indexed = std::path::Path::new(&request.folder).join(".codegraph/codegraph.db").is_file();
    if uses_codegraph(request) && indexed && !request.mcp.iter().any(|server| server.name == "codegraph") {
        request.mcp.push(crate::config::McpServerConfig {
            name: "codegraph".into(),
            command: "codegraph".into(),
            args: vec!["serve".into(), "--mcp".into()],
            env: Default::default(),
            enabled: true,
        });
    }
}

fn add_question_rules(request: &mut ProviderRequest) {
    if let Some(message) = request.messages.iter_mut().rev().find(|m| m.role == "user") {
        message.content = format!(
            "{}\n\n{}",
            crate::core::rule::chat_questions::RULE,
            message.content
        );
    }
}

/// Cancels the run when the browser drops the stream.
struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

/// The `provider_id` of the "MutiS" source: the server picks the lead model, so the request does not say which one runs.
const MUTIS_SOURCE: &str = "mutis";

fn should_remember_folder(current: &str, folder: &str, plain_chat: bool, dev_chat: bool) -> bool {
    !plain_chat && !dev_chat && current != folder
}

pub(crate) async fn chat_handler(
    State(state): State<AppState>,
    Json(mut request): Json<ChatRequest>,
) -> ApiResult<Response> {
    if !chat::valid_id(&request.chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    // A chat at the MutiS source runs on the lead role's source; only such a chat is a MutiS chat (delegation, rules, role label).
    // `mutis_level` is the effort the user picked for MutiS (max/xhigh = three AIs, else High); `request.effort` becomes the lead's own.
    let mutis_level = if request.provider_id == MUTIS_SOURCE {
        let level = request.effort.clone().unwrap_or_else(|| "max".into());
        let lead = crate::api::mutis::resolve_lead(&state, &level)
            .await
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::BAD_REQUEST,
                    "MutiS chưa có nguồn lead nào đang kết nối",
                )
            })?;
        request.file_tools = chat::file_tools_for(&lead.provider, request.permission);
        request.provider_id = lead.provider.id;
        request.model = lead.model;
        request.effort = Some(lead.effort);
        Some(level)
    } else {
        None
    };
    let existing = state.chats.get(&request.chat_id).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không thể đọc lịch sử chat",
        )
    })?;
    let dev_chat = request.dev_chat || existing.as_ref().is_some_and(|chat| chat.dev_chat);
    let folder = request::resolve_folder(&state, &request, dev_chat)?;
    let plain_chat = !dev_chat && request.folder.trim().is_empty();
    let current_folder = state.config.read().await.default_folder.clone();
    if should_remember_folder(&current_folder, &folder, plain_chat, dev_chat) {
        let mut config = state.config.write().await;
        if config.default_folder != folder {
            config.default_folder = folder.clone();
            if let Err(error) = crate::config::save(&state.config_path, &config) {
                eprintln!("Không lưu được thư mục làm việc gần đây: {error}");
            }
        }
    }
    let effort = request::check_request(&request)?;
    let mut provider_config = {
        let config = state.config.read().await;
        // Old chats may still point at the removed "DeepSeek làm việc" source: resolve it to the Deploy source.
        crate::config::resolve_provider_id(&config.providers, &request.provider_id)
            .cloned()
            .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?
    };
    // File tools = agent mode of the SAME source; Full also grants API-key sources run_command.
    let file_tools = if plain_chat {
        chat::FileTools::None
    } else {
        request.file_tools
    };
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
    let dev_preamble = dev_chat.then(|| crate::api::dev::dev_chat_preamble(&folder));
    let latest = &request
        .messages
        .last()
        .expect("validated user message")
        .content;
    // Older dev chats stored the preamble in the first user message.
    let user_content = dev_preamble
        .as_deref()
        .and_then(|preamble| latest.strip_prefix(preamble)?.strip_prefix("\n\n"))
        .unwrap_or(latest);
    let expanded_message =
        skills::expand_message(&state, user_content, &provider_config.kind).await?;

    let mutis_config = state.config.read().await.mutis.clone();
    // MutiS High + Codex: the app runs the delegated worker for Codex (see providers::delegate). Any source that is
    // active in Settings › Nguồn AI can be the worker, whatever its kind (API key or CLI).
    let role_effort_name = mutis_level.as_deref().unwrap_or("max");
    let delegate_worker = if matches!(provider_config.kind.as_str(), "codex_cli" | "claude_cli")
        && mutis_level.is_some()
        && request.plan_id.is_none()
        && !plain_chat
        && crate::config::MutisConfig::level_for(role_effort_name) == "high"
    {
        let providers_now = state.config.read().await.providers.clone();
        // High roles may be unset (old configs): fall back to the Max small worker.
        let high_small = &mutis_config.roles_for(role_effort_name).small;
        let role = if high_small.provider_id.trim().is_empty() {
            &mutis_config.roles.small
        } else {
            high_small
        };
        let configured_id = crate::config::resolve_provider_id(&providers_now, &role.provider_id)
            .map(|p| p.id.clone());
        let mut chosen = None;
        for provider in worker_candidates(&providers_now, &role.provider_id) {
            let active = crate::api::providers::cached_provider_test(&state, &provider.id)
                .await
                .is_ok_and(|test| test.status == "connected");
            let own_model = configured_id.as_deref() == Some(provider.id.as_str())
                && !role.model.trim().is_empty();
            let model = if own_model {
                role.model.trim().to_owned()
            } else {
                provider.models.first().cloned().unwrap_or_default()
            };
            if active && !model.is_empty() {
                chosen = Some((
                    provider,
                    model,
                    chat::parse_effort(&role.effort).unwrap_or_default(),
                ));
                break;
            }
        }
        chosen
    } else {
        None
    };
    // The optional independent checker: the High section's "checker" role, which must be a Codex source (it keeps and resumes its own thread).
    let (delegate_checker, checker_note) = if delegate_worker.is_some()
        && mutis_config.delegate_checker
    {
        let role = &mutis_config.high_checker;
        let config = state.config.read().await;
        let found = crate::config::resolve_provider_id(&config.providers, &role.provider_id)
            .filter(|p| p.kind == "codex_cli")
            .and_then(|p| {
                let model = if role.model.trim().is_empty() {
                    p.models.first().cloned().unwrap_or_default()
                } else {
                    role.model.trim().to_owned()
                };
                (!model.is_empty()).then(|| {
                    (
                        p.clone(),
                        model,
                        chat::parse_effort(&role.effort).unwrap_or(chat::Effort::Max),
                    )
                })
            });
        let note = found.is_none().then(|| "Người kiểm tra chưa chạy: vào Cài đặt › MutiS › Mức High, mục «Người kiểm tra», chọn nguồn Codex CLI và model.".to_owned());
        (found, note)
    } else {
        (None, None)
    };
    // Codex compacts a thread past the budget by itself (providers/codex/compact.rs). Only a thread at twice the budget, where
    // compacting did not help, starts afresh; the main one is rebuilt from the recent conversation.
    let mut existing = existing;
    let token_budget = state.config.read().await.thread_token_budget.saturating_mul(2);
    let rotated = existing
        .as_mut()
        .map(|r| r.rotate_big_threads(token_budget))
        .unwrap_or_default();
    let provider: Box<dyn providers::Provider> = match &delegate_worker {
        Some((worker, model, effort)) => {
            // A Codex worker's thread only resumes inside the Codex home it was made in.
            let kept = |id: &str| existing.as_ref().and_then(|r| r.worker_sessions.get(id).cloned());
            let mut worker_session = kept(&worker.id);
            let mut worker2_session = kept(&providers::delegate::worker2_key(&worker.id));
            if worker.kind == "codex_cli" {
                let settings = state.config.read().await.codex.clone();
                let prefix = providers::codex::session_prefix(&settings, worker, &folder);
                worker_session = worker_session.filter(|id| id.starts_with(&prefix));
                worker2_session = worker2_session.filter(|id| id.starts_with(&prefix));
            }
            Box::new(providers::delegate::DelegateProvider {
            worker: worker.clone(),
            model: model.clone(),
            effort: *effort,
            checker: delegate_checker.clone(),
            checker_note: checker_note.clone(),
            chat_id: request.chat_id.clone(),
            lead_kind: provider_config.kind.clone(),
            worker_session,
            worker2_session,
        })},
        None => providers::for_kind(&provider_config.kind).map_err(|_| {
            ApiError::new(StatusCode::BAD_REQUEST, "Loại nhà cung cấp không hợp lệ")
        })?,
    };
    // The thread belongs to the source and to the mode (MutiS lead or a single model), so the same Codex in the two modes never shares one.
    let thread = chat::thread_key(&request.provider_id, mutis_level.is_some());
    let switched_source = existing.as_ref().is_some_and(|r| r.thread_key() != thread);
    let mut record = existing.unwrap_or_else(|| chat::new_record(&request));
    if switched_source {
        record.switch_source(&thread);
    }
    if provider_config.kind == "codex_cli" {
        let settings = state.config.read().await.codex.clone();
        let prefix = providers::codex::session_prefix(&settings, &provider_config, &folder);
        if !record
            .session_id
            .as_deref()
            .is_some_and(|id| id.starts_with(&prefix))
        {
            record.session_id = None;
        }
        // The checker is Codex too, so its thread belongs to the same Codex home as the coordinator's.
        if !record
            .checker_session_id
            .as_deref()
            .is_some_and(|id| id.starts_with(&prefix))
        {
            record.checker_session_id = None;
        }
    }
    record.provider_id = request.provider_id.clone();
    record.mutis_level = mutis_level.clone();
    record.model = request.model.clone();
    record.folder = if plain_chat {
        String::new()
    } else {
        folder.clone()
    };
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
    record.messages.push(ChatMessage {
        debate_id: None,
        plan_id: None,
        role: "assistant".to_owned(),
        content: String::new(),
        reasoning: String::new(),
        logs: Vec::new(),
        diff: String::new(),
        files: Vec::new(),
        tasks: None,
        provider: avatar_source(&provider_config, &request.model),
        model: request.model.clone(),
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
    let files_before = if plain_chat {
        Default::default()
    } else {
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
        checker_session_id: record.checker_session_id.clone(),
    };
    if let Some(content) = expanded_message {
        if let Some(message) = provider_request.messages.last_mut() {
            message.content = content;
        }
    }
    if let Some(preamble) = dev_preamble {
        if let Some(first) = provider_request
            .messages
            .iter_mut()
            .find(|message| message.role == "user")
        {
            if !first.content.starts_with(&preamble) {
                first.content = format!("{preamble}\n\n{}", first.content);
            }
        }
    }
    if memory_config.enabled {
        crate::features::memory::inject(&mut provider_request, &memory_context);
    }
    // Plan workers run unattended; interactive chat supports user decision cards on every turn.
    // The delegated worker decides which protocol Codex gets: exact edits for DeepSeek, goals and limits for any other model.
    let worker_kind = delegate_worker.as_ref().map(|(worker, model, _)| {
        if worker.is_deepseek(model) {
            crate::core::rule::mutis_high_delegation::Worker::DeepSeek
        } else {
            crate::core::rule::mutis_high_delegation::Worker::Other
        }
    });
    if request.plan_id.is_none() {
        add_question_rules(&mut provider_request);
        if !plain_chat {
            let rule = crate::core::rule::mutis_high_delegation::for_chat(
                mutis_level.is_some(),
                crate::config::MutisConfig::level_for(role_effort_name),
                &provider_request.provider.kind,
                provider_request
                    .mcp
                    .iter()
                    .any(|server| server.name.contains("deepseek")),
                worker_kind,
            );
            if !rule.is_empty() {
                let rule = if delegate_checker.is_some() {
                    format!("{rule}\n{}", crate::core::rule::mutis_checker::NOTE)
                } else {
                    rule.to_owned()
                };
                let rule = if worker_kind
                    == Some(crate::core::rule::mutis_high_delegation::Worker::DeepSeek)
                    && !mutis_config.deepseek_ui
                {
                    format!(
                        "{rule}\n{}",
                        crate::core::rule::mutis_high_delegation::NO_UI_NOTE
                    )
                } else {
                    rule
                };
                if let Some(message) = provider_request
                    .messages
                    .iter_mut()
                    .rev()
                    .find(|m| m.role == "user")
                {
                    message.content = format!("{rule}\n\n{}", message.content);
                }
            }
        }
    }
    // A source that starts without its own session only gets a written copy of the earlier chat: say so, since it cannot see what others ran.
    let others = provider_request.messages.iter().any(|m| m.role == "assistant" && !m.provider.is_empty() && m.provider != provider_request.provider.kind);
    if provider_request.session_id.is_none() && others && provider_request.provider.kind.ends_with("_cli") {
        let _ = providers::emit_text(&provider_sender, "log", format!("Chuyển sang {}: AI này chỉ nhận bản chép hội thoại và danh sách file đã đổi, không thấy lệnh hay công cụ nguồn trước đã chạy.", provider_request.provider.name)).await;
    }
    // Plan task workers cannot stop to ask the user, so they skip the first-request rules.
    if request.plan_id.is_none() && !plain_chat && state.config.read().await.codegraph_enabled {
        add_first_request_rules(&mut provider_request, &provider_sender).await;
        attach_codegraph(&mut provider_request);
    }
    let event_provider_id = provider_request.provider.id.clone();
    let provider_name = provider_request.provider.name.clone();
    let provider_kind = avatar_source(&provider_request.provider, &provider_request.model);
    let status_model = provider_request.model.clone();
    let status_effort = format!("{:?}", provider_request.effort).to_lowercase();
    // A plan task has no MutiS level of its own: its effort picks the roles, as before.
    let role_effort = mutis_level
        .as_deref()
        .or(request.effort.as_deref())
        .unwrap_or("max");
    let status_role = (mutis_level.is_some() || request.plan_id.is_some())
        .then(|| {
            ["lead", "big", "small"]
                .into_iter()
                .find(|role| match *role {
                    "lead" => {
                        mutis_config.roles_for(role_effort).lead.provider_id == event_provider_id
                    }
                    "big" => {
                        mutis_config.roles_for(role_effort).big.provider_id == event_provider_id
                    }
                    _ => mutis_config.roles_for(role_effort).small.provider_id == event_provider_id,
                })
                .map(str::to_owned)
        })
        .flatten();
    for (key, tokens) in rotated {
        let which = match key.strip_prefix("worker:") {
            Some(worker) => format!("worker {worker}"),
            None => "chính".to_owned(),
        };
        let _ = providers::emit_text(
            &provider_sender,
            "log",
            format!("Thread {which} đã đọc {tokens} token (ngưỡng {token_budget}), mở thread mới."),
        )
        .await;
    }
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

    #[test]
    fn remembers_only_a_different_folder_for_non_plain_non_dev_chats() {
        assert!(should_remember_folder("old", "new", false, false));
        assert!(!should_remember_folder("same", "same", false, false));
        assert!(!should_remember_folder("old", "new", true, false));
        assert!(!should_remember_folder("old", "new", false, true));
    }

    fn source(id: &str, kind: &str, agent_capable: bool) -> crate::config::ProviderConfig {
        crate::config::ProviderConfig {
            id: id.into(),
            kind: kind.into(),
            agent_capable,
            ..Default::default()
        }
    }

    #[test]
    fn worker_candidates_start_with_the_role_source_whatever_its_kind() {
        let providers = [
            source("claude", "claude_cli", false),
            source("api", "openai_compat", true),
            source("codex", "codex_cli", false),
            source("plain", "anthropic", false),
        ];
        let ids = |configured: &str| {
            worker_candidates(&providers, configured)
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<_>>()
        };
        // A CLI role source is used as it is (no API key needed), then the agent API source and the Codex source follow.
        assert_eq!(ids("codex"), ["codex", "api"]);
        assert_eq!(ids("claude"), ["claude", "api", "codex"]);
        assert_eq!(ids("api"), ["api", "codex"]);
        // A kind that cannot run tasks, or an unknown id, is skipped; the others still apply.
        assert_eq!(ids("plain"), ["api", "codex"]);
        assert_eq!(ids("missing"), ["api", "codex"]);
    }

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
            checker_session_id: None,
        }
    }

    #[test]
    fn deepseek_models_get_their_own_avatar_label() {
        let source = |id: &str, kind: &str| crate::config::ProviderConfig {
            id: id.into(),
            kind: kind.into(),
            ..Default::default()
        };
        assert_eq!(
            avatar_source(
                &source("openai", "openai_compat"),
                "deepseek-ai/deepseek-v4"
            ),
            "deepseek"
        );
        assert_eq!(
            avatar_source(&source("deploy-deepseek", "mcp_agent"), "x"),
            "deepseek"
        );
        assert_eq!(
            avatar_source(&source("openai", "openai_compat"), "gpt-5.1"),
            "openai_compat"
        );
        assert_eq!(
            avatar_source(&source("codex-cli", "codex_cli"), "gpt-6-luna"),
            "codex_cli"
        );
    }

    #[tokio::test]
    async fn rules_go_only_on_the_first_user_message() {
        let (events, mut log) = mpsc::channel(8);
        let mut first = request(json!([{"role":"user","content":"làm đi"}]));
        first.provider.kind = "codex_cli".into();
        add_first_request_rules(&mut first, &events).await;
        assert!(first.messages[0].content.contains(".codegraph"));
        // A folder that does not exist is skipped quietly: nothing is built and no failure is logged.
        assert!(log.try_recv().is_err());
        assert!(first.messages[0].content.ends_with("làm đi"));
        // The app states the index status itself, so the AI does not spend a command (and a model call) on it.
        assert!(first.messages[0]
            .content
            .contains("DOES NOT HAVE a .codegraph folder"));
        let mut later = request(json!([
            {"role":"user","content":"a"},{"role":"assistant","content":"b"},{"role":"user","content":"c"}
        ]));
        later.provider.kind = "codex_cli".into();
        add_first_request_rules(&mut later, &events).await;
        assert!(later
            .messages
            .iter()
            .all(|m| !m.content.contains(".codegraph")));
        // API sources cannot call the tool, so they get neither the rule nor a failure note.
        let mut api = request(json!([{"role":"user","content":"làm đi"}]));
        api.provider.kind = "openai_compat".into();
        add_first_request_rules(&mut api, &events).await;
        assert_eq!(api.messages[0].content, "làm đi");
    }

    #[test]
    fn codegraph_tool_is_attached_once_to_indexed_cli_projects() {
        let dir = std::env::temp_dir().join(format!("mutis-cg-{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".codegraph")).unwrap();
        std::fs::write(dir.join(".codegraph/codegraph.db"), b"x").unwrap();
        let mut codex = request(json!([{"role":"user","content":"a"}]));
        codex.folder = dir.to_string_lossy().into_owned();
        codex.provider.kind = "codex_cli".into();
        attach_codegraph(&mut codex);
        attach_codegraph(&mut codex);
        assert_eq!(codex.mcp.iter().filter(|s| s.name == "codegraph").count(), 1);
        let mut api = request(json!([{"role":"user","content":"a"}]));
        api.folder = codex.folder.clone();
        api.provider.kind = "anthropic".into();
        attach_codegraph(&mut api);
        assert!(api.mcp.is_empty());
        let mut bare = request(json!([{"role":"user","content":"a"}]));
        bare.provider.kind = "claude_cli".into();
        attach_codegraph(&mut bare);
        assert!(bare.mcp.is_empty(), "no index, no tool");
        let _ = std::fs::remove_dir_all(dir);
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
        assert!(turn.messages[2]
            .content
            .starts_with(crate::core::rule::chat_questions::RULE));
        assert!(turn.messages[2].content.ends_with("follow up"));
    }
}
