mod delegate;
mod record;
mod resolve;
mod rules;
mod stream;

#[cfg(test)]
use self::delegate::worker_candidates;
use self::delegate::{build_provider, select_delegate};
#[cfg(test)]
use self::record::avatar_source;
use self::record::save_and_register;
#[cfg(test)]
use self::resolve::should_remember_folder;
use self::resolve::{resolve_request, Resolved};
use self::rules::apply_rules;
#[cfg(test)]
use self::rules::{
    add_first_request_rules, add_first_request_rules_if, add_question_rules, attach_codegraph,
};
use self::stream::start;
use super::skills;
use crate::{
    api::ApiResult,
    app::AppState,
    chat::ChatRequest,
    providers::{ProviderEvent, ProviderRequest},
};
use axum::{extract::State, response::Response, Json};
#[cfg(test)]
use serde_json::json;
use tokio::sync::mpsc;

pub(crate) async fn chat_handler(
    State(state): State<AppState>,
    Json(mut request): Json<ChatRequest>,
) -> ApiResult<Response> {
    let Resolved {
        mutis_level,
        existing,
        dev_chat,
        folder,
        plain_chat,
        effort,
        provider_config,
        file_tools,
        agent,
    } = resolve_request(&state, &mut request).await?;
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
    let role_effort_name = mutis_level.as_deref().unwrap_or("high");
    let (delegate_worker, delegate_checker, checker_note) = select_delegate(
        &state,
        &provider_config.kind,
        mutis_level.is_some(),
        request.plan_id.is_some(),
        plain_chat,
        &mutis_config,
        role_effort_name,
    )
    .await;
    // Codex compacts a thread past the budget by itself (providers/codex/compact.rs). Only a thread at twice the budget, where
    // compacting did not help, starts afresh; the main one is rebuilt from the recent conversation.
    let mut existing = existing;
    let token_budget = state.config.read().await.thread_token_budget.saturating_mul(2);
    let rotated = existing
        .as_mut()
        .map(|r| r.rotate_big_threads(token_budget))
        .unwrap_or_default();
    let provider = build_provider(
        &state,
        &provider_config.kind,
        &request.chat_id,
        &folder,
        existing.as_ref(),
        delegate_worker.as_ref(),
        &delegate_checker,
        &checker_note,
    )
    .await?;
    let (record, cancel, request_id) = save_and_register(
        &state,
        &request,
        existing,
        &provider_config,
        mutis_level.as_deref(),
        &folder,
        plain_chat,
        dev_chat,
        file_tools,
        effort,
    )
    .await?;
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
    apply_rules(
        &mut provider_request,
        &provider_sender,
        &state,
        request.plan_id.is_some(),
        plain_chat,
        mutis_level.is_some(),
        &mutis_config,
        role_effort_name,
        worker_kind,
        delegate_checker.is_some(),
    )
    .await;
    Ok(start(
        state,
        provider,
        provider_request,
        record,
        cancel,
        provider_cancel,
        request_id,
        sse_sender,
        sse_receiver,
        provider_sender,
        provider_receiver,
        mutis_config,
        memory_config,
        files_before,
        rotated,
        token_budget,
        mutis_level,
        request.effort,
        request.chat_id,
        request.plan_id,
        request.task_id,
        request.task_kind,
    )
    .await)
}

#[cfg(test)]
mod tests;
