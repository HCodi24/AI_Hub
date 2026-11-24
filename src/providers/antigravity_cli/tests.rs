use super::compact::{sizes, note_input};
use super::parser::Parser;
use super::permission::{mode, permission_rules};
use super::*;


fn run(lines: &[&str]) -> Vec<(String, String)> {
    let mut parser = Parser::default();
    lines.iter().flat_map(|line| parser.parse(line)).map(|e| (e.name.to_owned(), e.data.to_string())).collect()
}

#[test]
fn streams_text_separates_steps_and_reports_tokens() {
    let out = run(&[
        r#"{"event":"init","conversation_id":"abc-1"}"#,
        r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"agent_response","text_delta":"Một"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"tool","tool_name":"view_file"}}"#,
        r#"{"event":"step_update","step_update":{"step_index":3,"state":"ACTIVE","step_type":"agent_response","text_delta":"Hai"}}"#,
        r#"{"event":"result","result":{"status":"SUCCESS","response":"Một Hai","usage":{"input_tokens":10,"output_tokens":4,"thinking_tokens":2,"cache_read_tokens":0}}}"#,
    ]);
    let text = |name: &str| out.iter().filter(|(n, _)| n == name).map(|(_, d)| d.as_str()).collect::<Vec<_>>();
    assert_eq!(text("session"), [r#"{"session_id":"abc-1"}"#]);
    assert_eq!(text("delta"), [r#"{"text":"Một"}"#, r#"{"text":"\n\nHai"}"#]);
    assert!(text("log").iter().any(|log| log.contains("view_file")));
    assert!(text("log").iter().any(|log| log.contains("vào 10")));
    assert!(text("error").is_empty());
}

#[test]
fn failed_result_is_an_error_and_success_without_stream_falls_back_to_the_response() {
    let failed = run(&[r#"{"event":"result","result":{"status":"ERROR","error":"authentication required"}}"#]);
    assert!(failed.iter().any(|(n, d)| n == "error" && d.contains("authentication required")));
    let plain = run(&[r#"{"event":"result","result":{"status":"SUCCESS","response":"ok"}}"#]);
    assert!(plain.iter().any(|(n, d)| n == "delta" && d.contains("ok")));
}

/// Real `agy` call (uses the signed-in account and a little quota): `cargo test antigravity_live -- --ignored --nocapture`.
#[tokio::test]
#[ignore]
async fn antigravity_live_turn_streams_text_and_resumes_the_conversation() {
    let folder = std::env::temp_dir().join("ai-hub-agy-live");
    std::fs::create_dir_all(&folder).unwrap();
    let ask = |text: &str, session: Option<String>| ProviderRequest {
        provider: crate::config::ProviderConfig { id: "antigravity-cli".into(), kind: "antigravity_cli".into(), ..Default::default() },
        model: "gemini-4-neon-low".into(),
        folder: folder.to_string_lossy().into_owned(),
        permission: Permission::Read,
        effort: crate::providers::Effort::Default,
        messages: vec![serde_json::from_value(json!({"role": "user", "content": text})).unwrap()],
        session_id: session,
        mutis: crate::config::Config::seeded().mutis,
        mcp: Vec::new(),
        write_scope: Vec::new(),
        checker_session_id: None,
    };
    let mut session = None;
    let mut replies = Vec::new();
    for text in ["Remember the word PINEAPPLE. Reply only: noted", "What word did I ask you to remember? Reply with that word only."] {
        let (tx, mut rx) = mpsc::channel(256);
        let request = ask(text, session.clone());
        let run = tokio::spawn(async move { AntigravityCliProvider.run(request, tx, CancellationToken::new()).await });
        let mut reply = String::new();
        while let Some(event) = rx.recv().await {
            println!("{} {}", event.name, event.data);
            match event.name {
                "delta" => reply.push_str(event.data["text"].as_str().unwrap_or_default()),
                "session" => session = event.data["session_id"].as_str().map(str::to_owned),
                "error" => panic!("error event: {}", event.data),
                _ => {}
            }
        }
        run.await.unwrap().unwrap();
        replies.push(reply);
    }
    assert!(session.is_some_and(|id| id.starts_with("mutis-agy-")));
    assert!(replies[1].to_uppercase().contains("PINEAPPLE"), "second turn forgot the first: {replies:?}");
}

#[test]
fn denied_actions_offer_one_level_up_except_in_full_mode() {
    let line = r#"{"event":"result","result":{"status":"SUCCESS","response":"","denied_actions":[{"action":"command","display_name":"Bash"}]}}"#;
    for (permission, need) in [(Permission::Read, Some("write")), (Permission::Write, Some("full")), (Permission::Full, None)] {
        let events = Parser { permission, ..Default::default() }.parse(line);
        let denied = events.iter().find(|e| e.name == "permission_denied");
        assert_eq!(denied.map(|e| e.data["need"].as_str().unwrap()), need);
    }
    // A refused command is asked about by the run itself, so the result adds no rerun card.
    let mut parser = Parser { permission: Permission::Read, ..Default::default() };
    parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ERROR","tool_info":{"parameters":{"CommandLine":"echo hi > a.txt"},"error":{"message":"permission check failed for command"}}}}"#);
    assert_eq!(parser.denied, [("command(echo hi > a.txt)".to_owned(), "echo hi > a.txt".to_owned())]);
    assert!(parser.parse(line).iter().all(|e| e.name != "permission_denied"));
    // No failed step was reported: the command that never finished is the refused one.
    let mut parser = Parser { permission: Permission::Write, ..Default::default() };
    parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE","tool_info":{"parameters":{"CommandLine":"echo hi > b.txt"}}}}"#);
    assert!(parser.parse(line).iter().all(|e| e.name != "permission_denied"));
    assert_eq!(parser.denied, [("command(echo hi > b.txt)".to_owned(), "echo hi > b.txt".to_owned())]);
    // An MCP tool the mode refused is asked about the same way.
    let mut parser = Parser { permission: Permission::Write, ..Default::default() };
    parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ACTIVE","tool_info":{"parameters":{"ServerName":"jira","ToolName":"search"}}}}"#);
    parser.parse(line);
    assert_eq!(parser.denied, [("mcp(jira/search)".to_owned(), "MCP jira: search".to_owned())]);
    assert!(run(&[r#"{"event":"result","result":{"status":"SUCCESS","response":"ok","denied_actions":[]}}"#]).iter().all(|(n, _)| n != "permission_denied"));
}

#[test]
fn read_denies_file_writes_and_no_command_is_pre_allowed() {
    let rules = |permission| permission_rules(permission, &["command(npm test)".to_owned()])["permissions"].clone();
    assert_eq!(rules(Permission::Read)["deny"], json!(["write_file(*)"]));
    assert_eq!(rules(Permission::Write)["deny"], json!([]));
    let allow = |permission| rules(permission)["allow"].to_string();
    assert_eq!(allow(Permission::Read), r#"["mcp(codegraph/codegraph_explore)","command(npm test)"]"#);
    assert!(allow(Permission::Full).contains("mcp(codegraph/codegraph_explore)"));
    // A write refused by the Read rule offers the level above.
    let mut parser = Parser { permission: Permission::Read, ..Default::default() };
    parser.parse(r#"{"event":"step_update","step_update":{"step_type":"tool","state":"ERROR","tool_info":{"parameters":{"Path":"C:/a.txt"},"error":{"message":"permission check failed for write_file"}}}}"#);
    let events = parser.parse(r#"{"event":"result","result":{"status":"SUCCESS","response":"no"}}"#);
    assert!(events.iter().any(|e| e.name == "permission_denied" && e.data["need"] == "write"));
}

#[test]
fn the_budget_counts_growth_since_the_last_compaction() {
    let id = "test-conversation-size";
    note_input(id, 100_000);
    assert_eq!(crate::providers::codex::compact::needs_compaction(id, 150_000), None);
    note_input(id, 200_000);
    assert_eq!(crate::providers::codex::compact::needs_compaction(id, 150_000), Some(200_000));
    // After a compaction the running input stays high, but only the growth after it counts.
    sizes().lock().unwrap().insert(id.to_owned(), (210_000, 210_000));
    crate::providers::codex::compact::forget(id);
    note_input(id, 240_000);
    assert_eq!(crate::providers::codex::compact::needs_compaction(id, 150_000), None);
    assert_eq!(crate::providers::codex::compact::needs_compaction(id, 20_000), Some(30_000));
    crate::providers::codex::compact::forget(id);
}

#[test]
fn permissions_map_to_agy_modes() {
    assert_eq!(mode(Permission::Read), "accept-edits");
    assert_eq!(mode(Permission::Write), "accept-edits");
    assert_eq!(mode(Permission::Full), "auto");
}
