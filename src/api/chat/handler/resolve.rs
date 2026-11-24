use super::super::request;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRequest},
    config::ProviderConfig,
};
use axum::http::StatusCode;

/// The `provider_id` of the "MutiS" source: the server picks the lead model, so the request does not say which one runs.
const MUTIS_SOURCE: &str = "mutis";

pub(super) fn should_remember_folder(current: &str, folder: &str, plain_chat: bool, dev_chat: bool) -> bool {
    !plain_chat && !dev_chat && current != folder
}

pub(super) struct Resolved {
    pub(super) mutis_level: Option<String>,
    pub(super) existing: Option<chat::ChatRecord>,
    pub(super) dev_chat: bool,
    pub(super) folder: String,
    pub(super) plain_chat: bool,
    pub(super) effort: chat::Effort,
    pub(super) provider_config: ProviderConfig,
    pub(super) file_tools: chat::FileTools,
    pub(super) agent: Option<chat::Permission>,
}

pub(super) async fn resolve_request(
    state: &AppState,
    request: &mut ChatRequest,
) -> ApiResult<Resolved> {
    if !chat::valid_id(&request.chat_id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id phải là UUID hợp lệ",
        ));
    }
    // A chat at the MutiS source runs on the lead role's source; only such a chat is a MutiS chat (delegation, rules, role label).
    // `mutis_level` is the effort the user picked for MutiS (always High); `request.effort` becomes the lead's own.
    let mutis_level = if request.provider_id == MUTIS_SOURCE {
        let level = request.effort.clone().unwrap_or_else(|| "high".into());
        let lead = crate::api::mutis::resolve_lead(state, &level)
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
    let folder = request::resolve_folder(state, request, dev_chat)?;
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
    let effort = request::check_request(request)?;
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
    Ok(Resolved {
        mutis_level,
        existing,
        dev_chat,
        folder,
        plain_chat,
        effort,
        provider_config,
        file_tools,
        agent,
    })
}
