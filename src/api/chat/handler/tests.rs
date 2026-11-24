use super::*;

#[test]
fn status_roles_use_the_configured_muti_s_sources() {
    let mut config = crate::config::MutisConfig::default();
    config.lead.provider_id = "lead-source".into();
    config.worker.provider_id = "worker-source".into();
    config.checker.provider_id = "checker-source".into();

    for (id, role) in [
        ("lead-source", "lead"),
        ("worker-source", "worker"),
        ("checker-source", "checker"),
    ] {
        assert_eq!(stream::status_role(&config, id, true), Some(role));
        assert_eq!(stream::status_role(&config, id, false), None);
    }
    assert_eq!(stream::status_role(&config, "unknown", true), None);
    config.checker.provider_id = config.lead.provider_id.clone();
    assert_eq!(
        stream::status_role(&config, "lead-source", true),
        Some("lead")
    );
}

#[test]
fn saved_mutis_markers_round_trip() {
    for marker in ["high", "mutis"] {
        let saved = json!({
            "id": "chat",
            "title": "chat",
            "provider_id": "lead-source",
            "model": "model",
            "folder": "",
            "messages": [],
            "updated_at": 0,
            "mutis_level": marker
        });
        let record: crate::chat::ChatRecord = serde_json::from_value(saved).unwrap();
        assert_eq!(record.mutis_level.as_deref(), Some(marker));
        assert_eq!(serde_json::to_value(record).unwrap()["mutis_level"], json!(marker));
    }
}

#[test]
fn remembers_only_a_different_folder_for_non_plain_non_dev_chats() {
    assert!(should_remember_folder("old", "new", false, false));
    assert!(!should_remember_folder("same", "same", false, false));
    assert!(!should_remember_folder("old", "new", true, false));
    assert!(!should_remember_folder("old", "new", false, true));
}


fn request(messages: serde_json::Value) -> ProviderRequest {
    ProviderRequest {
        provider: Default::default(),
        model: "m".into(),
        folder: "f".into(),
        permission: Default::default(),
        effort: Default::default(),
        messages: serde_json::from_value(messages).unwrap(),
        session_id: None,
        mutis: Default::default(),
        mcp: Vec::new(),
        write_scope: Vec::new(),
        checker_session_id: None,
    }
}

#[test]
fn deepseek_models_get_their_own_avatar_label() {
    let source = |id: &str, kind: &str| crate::config::ProviderConfig {
        id: id.into(),
        kind: kind.into(),
        ..Default::default()
    };
    assert_eq!(
        avatar_source(
            &source("openai", "openai_compat"),
            "deepseek-ai/deepseek-v4"
        ),
        "deepseek"
    );
    assert_eq!(
        avatar_source(&source("deploy-deepseek", "mcp_agent"), "x"),
        "deepseek"
    );
    assert_eq!(
        avatar_source(&source("openai", "openai_compat"), "gpt-5.1"),
        "openai_compat"
    );
    assert_eq!(
        avatar_source(&source("codex-cli", "codex_cli"), "gpt-6-luna"),
        "codex_cli"
    );
}

#[tokio::test]
async fn rules_go_only_on_the_first_user_message() {
    let (events, mut log) = mpsc::channel(8);
    let mut first = request(json!([{"role":"user","content":"làm đi"}]));
    first.provider.kind = "codex_cli".into();
    add_first_request_rules(&mut first, &events).await;
    assert!(first.messages[0].content.contains(".codegraph"));
    let mut off = request(json!([{"role":"user","content":"làm đi"}]));
    off.provider.kind = "codex_cli".into();
    add_first_request_rules_if(&mut off, &events, true, true).await;
    assert!(off.messages[0].content.contains("AGENTS.md") && !first.messages[0].content.contains("AGENTS.md"));
    // A folder that does not exist is skipped quietly: nothing is built and no failure is logged.
    assert!(log.try_recv().is_err());
    assert!(first.messages[0].content.ends_with("làm đi"));
    // The app states the index status itself, so the AI does not spend a command (and a model call) on it.
    assert!(first.messages[0]
        .content
        .contains("DOES NOT HAVE a .codegraph folder"));
    let mut later = request(json!([
        {"role":"user","content":"a"},{"role":"assistant","content":"b"},{"role":"user","content":"c"}
    ]));
    later.provider.kind = "codex_cli".into();
    add_first_request_rules(&mut later, &events).await;
    assert!(later
        .messages
        .iter()
        .all(|m| !m.content.contains(".codegraph")));
    // API sources cannot call the tool, so they get neither the rule nor a failure note.
    let mut api = request(json!([{"role":"user","content":"làm đi"}]));
    api.provider.kind = "openai_compat".into();
    add_first_request_rules(&mut api, &events).await;
    assert_eq!(api.messages[0].content, "làm đi");
}

#[test]
fn codegraph_tool_is_attached_once_to_indexed_cli_projects() {
    let dir = std::env::temp_dir().join(format!("mutis-cg-{}", std::process::id()));
    std::fs::create_dir_all(dir.join(".codegraph")).unwrap();
    std::fs::write(dir.join(".codegraph/codegraph.db"), b"x").unwrap();
    let mut codex = request(json!([{"role":"user","content":"a"}]));
    codex.folder = dir.to_string_lossy().into_owned();
    codex.provider.kind = "codex_cli".into();
    attach_codegraph(&mut codex);
    attach_codegraph(&mut codex);
    assert_eq!(codex.mcp.iter().filter(|s| s.name == "codegraph").count(), 1);
    let mut api = request(json!([{"role":"user","content":"a"}]));
    api.folder = codex.folder.clone();
    api.provider.kind = "anthropic".into();
    attach_codegraph(&mut api);
    assert!(api.mcp.is_empty());
    let mut bare = request(json!([{"role":"user","content":"a"}]));
    bare.provider.kind = "claude_cli".into();
    attach_codegraph(&mut bare);
    assert!(bare.mcp.is_empty(), "no index, no tool");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn question_rules_reach_only_the_current_user_message() {
    let mut turn = request(json!([
        {"role":"user","content":"first"},
        {"role":"assistant","content":"reply"},
        {"role":"user","content":"follow up"}
    ]));
    add_question_rules(&mut turn);
    assert_eq!(turn.messages[0].content, "first");
    assert_eq!(turn.messages[1].content, "reply");
    assert!(turn.messages[2]
        .content
        .starts_with(crate::core::rule::chat_questions::RULE));
    assert!(turn.messages[2].content.ends_with("follow up"));
}
