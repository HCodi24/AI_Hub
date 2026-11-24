use super::{mpsc, scope::in_scope, AgentTools, CancellationToken, McpTools, PathBuf, Permission, Value};
use serde_json::json;

#[test]
fn scope_limits_edits_to_listed_files_and_new_files_beside_them() {
    let root = std::env::temp_dir().join(format!("ai-hub-scope-{}", std::process::id()));
    std::fs::create_dir_all(root.join("web/src")).unwrap();
    std::fs::write(root.join("web/src/other.ts"), "x").unwrap();
    let scope = vec!["web/src/a.tsx".to_owned()];
    assert!(in_scope(&root, &scope, "./web/src/A.tsx", false));
    assert!(!in_scope(&root, &scope, "web/src/other.ts", false), "unlisted existing file");
    assert!(!in_scope(&root, &scope, "web/src/new.tsx", false), "edit_file never creates");
    assert!(in_scope(&root, &scope, "web/src/new.tsx", true), "new file beside a listed one");
    assert!(!in_scope(&root, &scope, "web/src/other.ts", true), "write_file over an unlisted existing file");
    assert!(!in_scope(&root, &scope, "src/new.rs", true), "other folder");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn definitions_follow_file_and_command_permissions_for_both_apis() {
    let tools = |permission, files| {
        let mcp = tokio::runtime::Runtime::new().unwrap().block_on(async {
            let (events, _receiver) = mpsc::channel(1);
            McpTools::start(&[], ".", permission != Permission::Read, &events).await
        });
        AgentTools::new(PathBuf::from("."), permission, 400, files, mcp)
    };
    let read = tools(Permission::Read, true);
    let write = tools(Permission::Write, true);
    let full = tools(Permission::Full, true);
    let mcp_only = tools(Permission::Full, false);
    let openai_names = |definitions: Vec<Value>| {
        definitions
            .iter()
            .map(|tool| tool["function"]["name"].as_str().unwrap().to_owned())
            .collect::<Vec<String>>()
    };
    assert_eq!(
        openai_names(read.openai_definitions()),
        vec!["list_dir", "read_file", "read_files"]
    );
    assert_eq!(
        openai_names(write.openai_definitions()),
        vec!["list_dir", "read_file", "read_files", "edit_file", "write_file"]
    );
    assert_eq!(
        openai_names(full.openai_definitions()),
        vec![
            "list_dir",
            "read_file",
            "read_files",
            "edit_file",
            "write_file",
            "run_command"
        ]
    );
    assert!(mcp_only.openai_definitions().is_empty());
    assert_eq!(
        full.anthropic_definitions()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "list_dir",
            "read_file",
            "read_files",
            "edit_file",
            "write_file",
            "run_command"
        ]
    );
    assert!(mcp_only.anthropic_definitions().is_empty());
}

#[tokio::test]
async fn read_files_reads_several_files_in_one_call_and_a_long_file_is_cut_not_refused() {
    let dir = std::env::temp_dir().join(format!("ai-hub-read-files-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.txt"), "one\ntwo\n").unwrap();
    std::fs::write(dir.join("long.txt"), "x\n".repeat(30)).unwrap();
    let root = dir.canonicalize().unwrap();
    let (events, mut receiver) = mpsc::channel(32);
    let mcp = McpTools::start(&[], ".", false, &events).await;
    let mut tools = AgentTools::new(root, Permission::Write, 10, true, mcp);
    let cancel = CancellationToken::new();
    let answer = tools.execute("read_files", serde_json::json!({"files":[{"path":"a.txt"},{"path":"long.txt"},{"path":"missing.txt"}]}), &events, &cancel).await;
    assert!(answer.contains("=== a.txt ===\n1: one\n2: two"), "{answer}");
    assert!(answer.contains("showing 1-10") && answer.contains("start_line=11"), "{answer}");
    assert!(answer.contains("=== missing.txt ===\nerror:"), "one bad file must not hide the others: {answer}");
    assert!(tools.execute("read_files", serde_json::json!({"files":[]}), &events, &cancel).await.starts_with("error:"));
    let mut logs = Vec::new();
    while let Ok(event) = receiver.try_recv() {
        logs.push(event.data["text"].as_str().unwrap_or("").to_owned());
    }
    assert!(logs.iter().any(|line| line == "Đọc a.txt 1–2"), "{logs:?}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[tokio::test]
async fn run_command_is_rejected_below_full_and_in_mcp_only_mode() {
    let (events, _receiver) = mpsc::channel(1);
    let cancel = CancellationToken::new();
    for (permission, files, expected) in [
        (
            Permission::Read,
            true,
            "error: lệnh chỉ chạy được ở quyền Toàn quyền",
        ),
        (
            Permission::Write,
            true,
            "error: lệnh chỉ chạy được ở quyền Toàn quyền",
        ),
        (
            Permission::Full,
            false,
            "error: Công cụ không tồn tại: run_command",
        ),
    ] {
        let mcp = super::super::mcp::McpTools::start(&[], ".", true, &events).await;
        let mut tools = AgentTools::new(PathBuf::from("."), permission, 400, files, mcp);
        assert_eq!(
            tools
                .execute(
                    "run_command",
                    json!({"command":"echo no"}),
                    &events,
                    &cancel
                )
                .await,
            expected
        );
    }
}

#[test]
fn anthropic_file_tools_use_input_schema() {
    let mcp = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(async { McpTools::start(&[], ".", true, &mpsc::channel(1).0).await });
    let tools = AgentTools::new(PathBuf::from("."), Permission::Read, 400, true, mcp);
    let definitions = tools.anthropic_definitions();
    assert!(definitions
        .iter()
        .all(|tool| tool["input_schema"].is_object()));
    assert!(definitions
        .iter()
        .all(|tool| tool.get("parameters").is_none()));
}
