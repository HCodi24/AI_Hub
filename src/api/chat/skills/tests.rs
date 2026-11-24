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
