mod auth;
mod command;
mod env;
mod jsonl;
mod prompt;
mod usage;

use super::approvals;
pub use super::discovery::resolve_executable;
#[cfg(test)]
pub use super::discovery::version_dir_key;
pub use crate::core::text::one_line;
pub use auth::{auth_origin, connected_message, parse_auth_account, set_auth_origin, AuthAccount};
pub use command::{command_for, searched_locations};
pub(crate) use command::terminate_child_tree;
pub use env::{
    agy_environment, ai_hub_claude_config_dir, claude_config_dir, clean_claude_environment,
    environment_names_to_remove,
};
pub use jsonl::{run_jsonl, Interact, JsonlCommand, Reaction};
pub use prompt::compose_prompt_for;
pub use usage::token_usage_log;

#[cfg(test)]
mod tests;

pub fn valid_model(model: &str) -> bool {
    !model.is_empty()
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}

pub(crate) fn valid_provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
}

pub fn safe_session_id(session_id: &str) -> bool {
    !session_id.is_empty()
        && !session_id.starts_with('-')
        && session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:/-".contains(&byte))
}
