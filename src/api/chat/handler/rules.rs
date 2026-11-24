use crate::{
    app::AppState,
    config::MutisConfig,
    core::rule::mutis_high_delegation::Worker,
    providers::{self, ProviderEvent, ProviderRequest},
};
use tokio::sync::mpsc;

/// Only the CLI sources get CodeGraph's MCP tool from the app; API sources and the DeepSeek agent cannot call it.
fn uses_codegraph(request: &ProviderRequest) -> bool {
    matches!(request.provider.kind.as_str(), "codex_cli" | "claude_cli" | "antigravity_cli")
}

/// First user message of the conversation gets the fixed rules in front of it. Before that the app builds or syncs
/// the project's CodeGraph index; problems go to the chat's activity log through `events`.
#[cfg(test)]
pub(super) async fn add_first_request_rules(request: &mut ProviderRequest, events: &mpsc::Sender<ProviderEvent>) {
    add_first_request_rules_if(request, events, true, false).await;
}

/// `app_rules` off (MutiS lead switch): the first message is left as it is.
pub(super) async fn add_first_request_rules_if(request: &mut ProviderRequest, events: &mpsc::Sender<ProviderEvent>, app_rules: bool, skip_guides: bool) {
    if !app_rules || !uses_codegraph(request) || request.messages.iter().filter(|m| m.role == "user").count() != 1 {
        return;
    }
    let prepared = crate::features::extensions::prepare(std::path::Path::new(&request.folder)).await;
    for note in prepared.notes {
        let _ = providers::emit_text(events, "log", note).await;
    }
    if let Some(message) = request.messages.iter_mut().find(|m| m.role == "user") {
        let guides = if skip_guides { format!("\n{}", crate::core::rule::project_guides::RULE) } else { String::new() };
        message.content = format!(
            "{}{guides}\n\n{}",
            crate::core::rule::first_request_rules(prepared.indexed, prepared.synced),
            message.content
        );
    }
}

/// Gives the AI CodeGraph's MCP server (every turn, so a resumed session keeps the tool) once the project has an index.
pub(super) fn attach_codegraph(request: &mut ProviderRequest) {
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

pub(super) fn add_question_rules(request: &mut ProviderRequest) {
    if let Some(message) = request.messages.iter_mut().rev().find(|m| m.role == "user") {
        message.content = format!(
            "{}\n\n{}",
            crate::core::rule::chat_questions::RULE,
            message.content
        );
    }
}

pub(super) async fn apply_rules(
    provider_request: &mut ProviderRequest,
    provider_sender: &mpsc::Sender<ProviderEvent>,
    state: &AppState,
    is_plan_task: bool,
    plain_chat: bool,
    is_mutis: bool,
    mutis_config: &MutisConfig,
    role_effort_name: &str,
    worker_kind: Option<Worker>,
    has_delegate_checker: bool,
) {
    if !is_plan_task {
        add_question_rules(provider_request);
        if !plain_chat {
            let rule = crate::core::rule::mutis_high_delegation::for_chat(
                is_mutis,
                crate::config::MutisConfig::level_for(role_effort_name),
                &provider_request.provider.kind,
                provider_request
                    .mcp
                    .iter()
                    .any(|server| server.name.contains("deepseek")),
                worker_kind,
            );
            if !rule.is_empty() {
                let rule = if has_delegate_checker {
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
    let others = provider_request.messages.iter().any(|m| {
        m.role == "assistant"
            && !m.provider.is_empty()
            && m.provider != provider_request.provider.kind
    });
    if provider_request.session_id.is_none()
        && others
        && provider_request.provider.kind.ends_with("_cli")
    {
        let _ = providers::emit_text(
            provider_sender,
            "log",
            format!(
                "Chuyển sang {}: AI này chỉ nhận bản chép hội thoại và danh sách file đã đổi, không thấy lệnh hay công cụ nguồn trước đã chạy.",
                provider_request.provider.name
            ),
        )
        .await;
    }
    // Plan task workers cannot stop to ask the user, so they skip the first-request rules.
    let skip_guides = if !is_plan_task && !plain_chat {
        let config = state.config.read().await;
        config
            .codegraph_enabled
            .then(|| match provider_request.provider.kind.as_str() {
                "codex_cli" => !config.codex.project_instructions,
                "claude_cli" => !config.claude.project_instructions,
                "antigravity_cli" => !config.antigravity.project_instructions,
                _ => false,
            })
    } else {
        None
    };
    if let Some(skip_guides) = skip_guides {
        add_first_request_rules_if(
            provider_request,
            provider_sender,
            !is_mutis || mutis_config.lead_app_rules,
            skip_guides,
        )
        .await;
        attach_codegraph(provider_request);
    }
    // The user's own rules for the MutiS lead go in front of the first message, after the app's.
    if is_mutis && !is_plan_task && !plain_chat && !mutis_config.lead_rules.trim().is_empty()
        && provider_request.messages.iter().filter(|m| m.role == "user").count() == 1
    {
        if let Some(message) = provider_request.messages.iter_mut().find(|m| m.role == "user") {
            message.content = format!(
                "Rules from the user:\n{}\n\n{}",
                mutis_config.lead_rules.trim(),
                message.content
            );
        }
    }
}
