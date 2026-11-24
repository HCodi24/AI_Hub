use super::*;
#[tokio::test]
async fn claude_settings_persist_without_credentials_and_reject_busy_changes() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/claude-api-test")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("config.json");
    let config = crate::config::Config::seeded();
    crate::config::save(&path, &config).unwrap();
    let state = AppState::new(config, path.clone(), root.join("chats"), None, 7788);
    let id = "claude-cli".to_owned();
    let response = claude_settings(
        State(state.clone()),
        Path(id.clone()),
        Json(ClaudeConfig {
            project_skills: true,
            ..Default::default()
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(response.project_skills && !response.project_instructions && !response.connected);
    let response = claude_settings(
        State(state.clone()),
        Path(id.clone()),
        Json(ClaudeConfig {
            project_instructions: true,
            ..Default::default()
        }),
    )
    .await
    .unwrap()
    .0;
    assert!(!response.project_skills && response.project_instructions);
    let saved: crate::config::Config =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(saved.claude.revision, 2);
    assert!(!saved.claude.project_skills && saved.claude.project_instructions);
    state
        .active_jobs
        .store(1, std::sync::atomic::Ordering::SeqCst);
    assert!(claude_settings(
        State(state.clone()),
        Path(id),
        Json(ClaudeConfig::default())
    )
    .await
    .is_err());
    assert_eq!(state.config.read().await.claude.revision, 2);
}
