use super::ProviderInput;
use crate::api::{ApiError, ApiResult};
use axum::http::StatusCode;

pub(super) fn validate_provider(id: &str, input: &ProviderInput) -> ApiResult<()> {
    if !valid_provider_id(id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã nhà cung cấp không hợp lệ",
        ));
    }
    if input.name.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tên nhà cung cấp không được để trống",
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "openai_compat" | "anthropic" | "codex_cli" | "claude_cli" | "antigravity_cli"
    ) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nhà cung cấp không hợp lệ",
        ));
    }
    if input.kind == "openai_compat" && input.base_url.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "OpenAI-compatible cần URL cơ sở",
        ));
    }
    Ok(())
}

pub(super) fn valid_provider_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 80
        && id.chars().all(|character| {
            character.is_ascii_alphanumeric() || character == '-' || character == '_'
        })
}

pub(super) fn clean_models(models: Vec<String>) -> Vec<String> {
    models
        .into_iter()
        .map(|model| model.trim().to_owned())
        .filter(|model| !model.is_empty())
        .collect()
}
