use super::input::ConfigInput;
use crate::config::Config;

pub(super) fn apply(config: &mut Config, input: ConfigInput) {
    if let Some(options) = input.antigravity {
        if let Some(enabled) = options.project_skills {
            config.antigravity.project_skills = enabled;
        }
        if let Some(enabled) = options.project_instructions {
            config.antigravity.project_instructions = enabled;
        }
    }
    if let Some(memory) = input.memory {
        config.memory = memory;
    }
    if let Some(folder) = input.default_folder {
        config.default_folder = folder.trim().to_owned();
    }
    if let Some(theme) = input.theme {
        config.theme = theme;
    }
    if let Some(style) = input.style {
        config.style = style;
    }
    if let Some(servers) = input.mcp_servers {
        config.mcp_servers = servers;
        config.codex.revision = config.codex.revision.wrapping_add(1);
    }
    if let Some(pets) = input.pets {
        config.pets = pets;
    }
    if let Some(on) = input.pets_enabled {
        config.pets_enabled = on;
    }
    if let Some(on) = input.codegraph_enabled {
        config.codegraph_enabled = on;
    }
    if let Some(on) = input.claude_usage_api {
        config.claude_usage_api = on;
    }
    if let Some(minutes) = input.chat_idle_timeout_minutes {
        config.chat_idle_timeout_minutes = minutes;
    }
    if let Some(tokens) = input.thread_token_budget {
        config.thread_token_budget = tokens;
    }
    if let Some(seconds) = input.approval_wait_seconds {
        config.approval_wait_seconds = seconds;
    }
    if let Some(provider_id) = input.planner_provider_id {
        config.planner_provider_id = provider_id;
        config.mutis.lead.provider_id = config.planner_provider_id.clone();
        config.mutis.reviewer_provider_id = config.planner_provider_id.clone();
    }
    if let Some(model) = input.planner_model {
        config.planner_model = model;
        config.mutis.lead.model = config.planner_model.clone();
        config.mutis.reviewer_model = config.planner_model.clone();
    }
}
