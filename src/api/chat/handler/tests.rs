use super::*;

#[test]
fn remembers_only_a_different_folder_for_non_plain_non_dev_chats() {
    assert!(should_remember_folder("old", "new", false, false));
    assert!(!should_remember_folder("same", "same", false, false));
    assert!(!should_remember_folder("old", "new", true, false));
    assert!(!should_remember_folder("old", "new", false, true));
}

fn source(id: &str, kind: &str, agent_capable: bool) -> crate::config::ProviderConfig {
    crate::config::ProviderConfig {
        id: id.into(),
        kind: kind.into(),
        agent_capable,
        ..Default::default()
    }
}

#[test]
fn worker_candidates_start_with_the_role_source_whatever_its_kind() {
    let providers = [
        source("claude", "claude_cli", false),
        source("api", "openai_compat", true),
        source("codex", "codex_cli", false),
        source("plain", "anthropic", false),
    ];
    let ids = |configured: &str| {
        worker_candidates(&providers, configured)
            .into_iter()
            .map(|p| p.id)
            .collect::<Vec<_>>()
    };
    // A CLI role source is used as it is (no API key needed), then the agent API source and the Codex source follow.
    assert_eq!(ids("codex"), ["codex", "api"]);
    assert_eq!(ids("claude"), ["claude", "api", "codex"]);
    assert_eq!(ids("api"), ["api", "codex"]);
    // A kind that cannot run tasks, or an unknown id, is skipped; the others still apply.
    assert_eq!(ids("plain"), ["api", "codex"]);
    assert_eq!(ids("missing"), ["api", "codex"]);
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
