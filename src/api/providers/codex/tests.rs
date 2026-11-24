use super::*;
#[test]
fn login_urls_are_official_https_only() {
    assert!(allowed_url("https://auth.openai.com/oauth/authorize?a=1"));
    for url in [
        "http://auth.openai.com",
        "https://auth.openai.com.evil.test",
        "javascript:alert(1)",
        "https://user@chatgpt.com",
    ] {
        assert!(!allowed_url(url));
    }
}

#[tokio::test]
async fn source_switches_persist_independently_without_authentication() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/codex-api-test")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.json");
    let config = crate::config::Config::seeded();
    crate::config::save(&path, &config).unwrap();
    let state = AppState::new(config, path.clone(), root.join("chats"), None, 0);
    let Json(initial) = codex_status(State(state.clone()), Path("codex-cli".into()))
        .await
        .unwrap();
    assert!(
        !initial.connected
            && !initial.pending
            && !initial.project_skills
            && !initial.project_instructions
    );
    let Json(updated) = codex_settings(
        State(state.clone()),
        Path("codex-cli".into()),
        Json(crate::config::CodexConfig {
            project_skills: true,
            project_instructions: false,
            revision: 900,
        }),
    )
    .await
    .unwrap();
    assert!(updated.project_skills && !updated.project_instructions);
    let stored: crate::config::Config =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(stored.codex.revision, 1);
    assert!(stored.codex.project_skills && !stored.codex.project_instructions);
    assert!(codex_action(
        State(state),
        Path("codex-cli".into()),
        Json(Action {
            action: "unexpected".into()
        })
    )
    .await
    .is_err());
}
