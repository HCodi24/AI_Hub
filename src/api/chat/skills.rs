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

/// Work from the raw user message, before any chat rules or memory are added.
pub(super) async fn expand_message(
    state: &AppState,
    content: &str,
    provider_kind: &str,
) -> ApiResult<Option<String>> {
    let name = match slash_command(content) {
        Some(name) if addons::valid_skill_name(name) => Some(name.to_owned()),
        Some(_) => return Ok(None),
        None if matches!(provider_kind, "claude_cli" | "codex_cli") => None,
        None => return Ok(None),
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
        Ok(format!(
            "Skill MutiS «/{name}» được chọn cho yêu cầu này. Tài nguyên đi kèm nằm trong «{}». Hãy dùng công cụ và quyền hiện có của phiên chat.\n\n<mutis-skill>\n{}\n</mutis-skill>",
            skill.directory.display(),
            skill.content
        ))
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được skill của MutiS",
        )
    })?
    .map_err(|error| ApiError::new(StatusCode::BAD_REQUEST, format!("{error:#}")))?;
    Ok((!context.is_empty()).then(|| format!("{context}\n\nYêu cầu của người dùng\n{content}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cli_sources_expand_the_same_skill_and_pass_through_other_slash_commands() {
        let root = std::env::temp_dir().join(format!("mutis-chat-skill-{}", uuid::Uuid::new_v4()));
        let directory = root.join("skills/review");
        std::fs::create_dir_all(&directory).unwrap();
        let root = crate::core::text::strip_verbatim(root.canonicalize().unwrap());
        let markdown = "# Rà soát\r\n\r\nTài nguyên nằm trong references.";
        std::fs::write(directory.join("SKILL.md"), markdown).unwrap();
        let state = AppState::new(
            crate::config::Config::seeded(),
            root.join("config.json"),
            root.join("chats"),
            None,
            0,
        );

        let message = "/review   rà soát nguyên văn\r\n\n  đoạn tiếp theo  ";
        let claude = expand_message(&state, message, "claude_cli")
            .await
            .unwrap()
            .unwrap();
        let codex = expand_message(&state, message, "codex_cli")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claude, codex);
        assert!(claude.contains(markdown));
        let directory = crate::core::text::strip_verbatim(directory.canonicalize().unwrap());
        assert!(claude.contains(directory.to_str().unwrap()));
        assert!(claude.ends_with(message));
        for other in ["/help", "/ review", "/plugin:review", "/review/file"] {
            assert!(expand_message(&state, other, "codex_cli")
                .await
                .unwrap()
                .is_none());
        }
        let catalogue = expand_message(&state, "Dùng skill phù hợp", "claude_cli")
            .await
            .unwrap()
            .unwrap();
        assert!(catalogue.contains("SKILL.md"));
        assert!(!catalogue.contains(markdown));
        std::fs::remove_dir_all(root).unwrap();
    }
}
