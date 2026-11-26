use super::input::ConfigInput;
use crate::{
    api::{ApiError, ApiResult},
    config,
};
use axum::http::StatusCode;

pub(super) fn validate(input: &ConfigInput) -> ApiResult<()> {
    if input
        .chat_idle_timeout_minutes
        .is_some_and(|minutes| minutes > 120)
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thời gian chờ chat phải từ 0 đến 120 phút",
        ));
    }
    if input.approval_wait_seconds.is_some_and(|seconds| !(10..=3600).contains(&seconds)) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thời gian chờ bạn trả lời phải từ 10 đến 3600 giây",
        ));
    }
    // 0 = never rotate; below 20k a thread would restart on almost every message.
    if input
        .thread_token_budget
        .is_some_and(|tokens| tokens != 0 && !(20_000..=2_000_000).contains(&tokens))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Ngưỡng token của thread phải là 0 (tắt) hoặc từ 20.000 đến 2.000.000",
        ));
    }
    if input
        .theme
        .as_deref()
        .is_some_and(|theme| !matches!(theme, "light" | "dark" | "system"))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Giao diện không hợp lệ",
        ));
    }
    if input
        .style
        .as_deref()
        .is_some_and(|style| !matches!(style, "default" | "glass"))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Phong cách giao diện không hợp lệ",
        ));
    }
    if let Some(pets) = &input.pets {
        if pets.iter().any(|(kind, pet)| {
            !valid_pet_kind(kind)
                || !(valid_pet(pet) || (pet.is_empty() && kind.starts_with("antigravity-")))
        })
        {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Lựa chọn pet không hợp lệ",
            ));
        }
    }
    if let Some(servers) = &input.mcp_servers {
        config::validate_mcp_servers(servers)
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    }
    if let Some(memory) = &input.memory {
        memory
            .validate()
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    }
    Ok(())
}

fn valid_pet_kind(kind: &str) -> bool {
    matches!(kind, "antigravity" | "deepseek")
        || (kind.len() <= 256
            && !kind.chars().any(char::is_control)
            && ["codex-", "claude-", "antigravity-"]
                .iter()
                .any(|prefix| kind.starts_with(*prefix)))
}

fn valid_pet(pet: &str) -> bool {
    (1..=48).contains(&pet.len())
        && pet
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
