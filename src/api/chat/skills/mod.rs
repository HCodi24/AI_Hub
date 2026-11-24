use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::addons,
};
use axum::http::StatusCode;

fn slash_command(content: &str) -> Option<&str> {
    content
        .trim_start()
        .strip_prefix('/')?
        .split(char::is_whitespace)
        .next()
}

fn skill_context(name: &str, directory: &std::path::Path, content: &str) -> String {
    format!(
        "Skill MutiS «/{name}» được chọn cho yêu cầu này. Tài nguyên đi kèm nằm trong «{}». Hãy dùng công cụ và quyền hiện có của phiên chat.\n\n<mutis-skill>\n{}\n</mutis-skill>",
        directory.display(),
        content
    )
}

/// Work from the raw user message, before any chat rules or memory are added.
pub(super) async fn expand_message(
    state: &AppState,
    content: &str,
    provider_kind: &str,
) -> ApiResult<Option<String>> {
    let name = match slash_command(content) {
        Some(name) if addons::valid_skill_name(name) => Some(name.to_owned()),
        Some(_) => return Ok(None),
        None => {
            if !matches!(provider_kind, "claude_cli" | "codex_cli") {
                return Ok(None);
            }
            None
        }
    };
    let providers = if name.is_some() {
        state.config.read().await.providers.clone()
    } else {
        Vec::new()
    };
    let config_path = state.config_path.clone();
    let context = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let Some(name) = name else {
            return Ok(addons::shared_skill_catalogue(&config_path));
        };
        let Some(skill) = addons::load_skill(&config_path, &providers, &name)? else {
            return Ok(String::new());
        };
        Ok(skill_context(&name, &skill.directory, &skill.content))
    })
    .await
    .map_err(|error| {
        eprintln!("Could not read skill: {error}");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được skill của MutiS",
        )
    })?
    .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, format!("{error:#}")))?;
    Ok((!context.is_empty()).then(|| format!("{context}\n\nYêu cầu của người dùng\n{content}")))
}

#[cfg(test)]
mod tests;
