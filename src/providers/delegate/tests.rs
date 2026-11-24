use super::events::{check_id, checker_activity, checker_progress, input_tokens, is_token_line, worker_log};
use super::live::cli_mcp;
use super::parse::{is_ui_file, guard_ui};
use super::report::{preload, worker_prompt, PRELOAD_MAX_LINES};
use super::*;
use crate::config::{McpServerConfig, MutisConfig};

#[test]
fn worker_token_lines_stay_unprefixed_so_the_token_table_counts_them() {
    assert_eq!(worker_log("Token: vào 10 (cache 2), ra 3 (suy luận 0)", "DeepSeek"), "Token: vào 10 (cache 2), ra 3 (suy luận 0)");
    assert_eq!(worker_log("Sửa a.rs", "DeepSeek"), "DeepSeek: Sửa a.rs");
    assert_eq!(worker_log("Sửa a.rs", "Tester"), "Tester: Sửa a.rs");
    assert!(is_token_line("Token: vào 1, ra 2") && !is_token_line("DeepSeek: Token: vào 1, ra 2"));
    assert_eq!(input_tokens("Token: vào 147202 (cache 145408), ra 2478"), Some(147202));
    assert_eq!(input_tokens("Codex: Command: x"), None);
}

#[test]
fn ui_tasks_never_go_to_deepseek_while_the_switch_is_off() {
    let task = |title: &str, files: &[&str], deps: &[&str]| Delegation {
        title: title.into(), files: files.iter().map(|f| f.to_string()).collect(), instructions: "i".into(), acceptance: vec![],
        depends_on: deps.iter().map(|d| d.to_string()).collect(), hard: false, trivial: false,
    };
    assert!(is_ui_file("web/src/a.CSS") && is_ui_file("x.scss") && is_ui_file("page.html") && !is_ui_file("web/src/a.tsx") && !is_ui_file("web/src/a.ts") && !is_ui_file("src/a.rs") && !is_ui_file("Makefile"));
    let tasks = || vec![task("logic", &["src/a.rs"], &[]), task("layout", &["web/a.css", "src/b.rs"], &[]), task("after", &["src/c.rs"], &["Layout"])];
    // Switch on: untouched.
    let (kept, notes) = guard_ui(tasks(), true);
    assert_eq!((kept.len(), notes.len()), (3, 0));
    // Off: the UI task and what depends on it go back to the coordinator.
    let (kept, notes) = guard_ui(tasks(), false);
    assert_eq!(kept.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["logic"]);
    assert_eq!(notes.len(), 2);
    // A task the coordinator already marked hard is left alone: it does it itself.
    let hard = Delegation { hard: true, ..task("layout", &["a.css"], &[]) };
    assert_eq!(guard_ui(vec![hard], false).0.len(), 1);
}

#[test]
fn extracts_valid_blocks_and_reports_broken_ones() {
    let text = "Plan.\n```mutis-delegate\n{\"title\":\"A\",\"files\":[\"src/a.rs\"],\"instructions\":\"do a\",\"acceptance\":[\"x\"]}\n```\nand\n```mutis-delegate\n{not json}\n```\n```mutis-delegate\n{\"title\":\"B\",\"instructions\":\"do b\"}\n```";
    let (tasks, errors) = extract(text);
    assert_eq!(tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(tasks[0].files, ["src/a.rs"]);
    assert_eq!(errors.len(), 1);
    assert!(extract("no blocks here").0.is_empty());
    assert_eq!(extract("```mutis-delegate\n{\"title\":\" \",\"instructions\":\"x\"}\n```").1.len(), 1);
}

#[test]
fn report_flags_a_worker_that_changed_nothing() {
    let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
    let idle = WorkerResult { summary: "I only read files".into(), diff: String::new(), error: None, refused: Vec::new() };
    let text = report(&task, "DeepSeek", 2, 4, 1, &idle);
    assert!(text.contains("lần 2/4") && text.contains("KHÔNG") && text.contains("còn 2 lần"));
    let edited = WorkerResult { summary: "done".into(), diff: "+ new line".into(), error: Some("boom".into()), refused: Vec::new() };
    let text = report(&task, "DeepSeek", 4, 4, 0, &edited);
    assert!(text.contains("LỖI: boom") && text.contains("+ new line") && text.contains("còn 0 lần"));
}

#[test]
fn report_headings_are_vietnamese_and_keep_the_evidence() {
    let task = Delegation { title: "Sửa đăng nhập".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
    let edited = WorkerResult { summary: "đã sửa".into(), diff: "+ let a = 1;".into(), error: None, refused: Vec::new() };
    let text = report(&task, "Worker 2", 1, 3, 2, &edited);
    // App-generated headings are Vietnamese.
    assert!(text.contains("Báo cáo của worker Worker 2"), "{text}");
    assert!(text.contains("Trạng thái: đã xong") && text.contains("Các file đã đổi: có, diff:"));
    assert!(text.contains("Tóm tắt của worker:") && text.contains("Ứng dụng đã tự chạy lại worker 2 lần"));
    // Identifiers, the attempt counter and the evidence stay verbatim.
    assert!(text.contains("\"Sửa đăng nhập\"") && text.contains("lần 1/3") && text.contains("còn 2 lần"));
    assert!(text.contains("+ let a = 1;") && text.contains("đã sửa"));
    // No English heading survives.
    for old in ["worker report", "Status:", "Files changed:", "Worker summary:", "attempts left", "NONE -"] {
        assert!(!text.contains(old), "old English label {old} in {text}");
    }
}

#[test]
fn app_retries_only_runs_that_changed_nothing() {
    let run = |diff: &str, error: Option<&str>, refused: &[&str]| WorkerResult {
        summary: String::new(), diff: diff.into(), error: error.map(String::from), refused: refused.iter().map(|s| s.to_string()).collect(),
    };
    assert!(auto_retry_reason(&run("+x", None, &[])).is_none());
    assert!(auto_retry_reason(&run("+x", Some("late error"), &[])).is_none(), "a partial change goes to the coordinator");
    assert_eq!(auto_retry_reason(&run("", None, &[])).as_deref(), Some("it changed no file"));
    let why = auto_retry_reason(&run("", Some("boom"), &["a.rs (ngoài phạm vi được giao)"])).unwrap();
    assert!(why.contains("failed with: boom") && why.contains("a.rs"));
}

#[test]
fn check_files_accepts_real_files_and_new_files_in_real_folders_only() {
    let root = std::env::temp_dir().join(format!("ai-hub-check-{}", std::process::id()));
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(root.join("web/a.ts"), "x").unwrap();
    let folder = root.to_str().unwrap();
    let ok = |files: &[&str]| check_files(folder, &files.iter().map(|f| f.to_string()).collect::<Vec<_>>()).is_empty();
    assert!(ok(&["web/a.ts", "web/new.ts"]));
    assert!(!ok(&["nope/new.ts"]) && !ok(&["../x.ts"]) && !ok(&["C:/x.ts"]) && !ok(&[]));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn only_cli_roles_get_the_codegraph_tool_and_only_when_the_lead_has_it() {
    let server = McpServerConfig { name: "codegraph".into(), command: "codegraph".into(), args: vec![], env: Default::default(), enabled: true };
    for kind in ["codex_cli", "claude_cli"] {
        assert_eq!(cli_mcp(Some(&server), kind).len(), 1);
        assert!(cli_mcp(None, kind).is_empty(), "no index or option off: no tool");
    }
    assert!(cli_mcp(Some(&server), "openai_compat").is_empty(), "an API worker cannot call MCP");
}

#[test]
fn worker_prompt_carries_the_task_files_and_criteria() {
    let task = Delegation { title: "T".into(), files: vec!["a.rs".into(), "b.rs".into()], instructions: "change x".into(), acceptance: vec!["builds".into()], depends_on: vec![], hard: false, trivial: false };
    let prompt = worker_prompt(&MutisConfig::default(), &task, "FILE: a.rs\nfn a() {}\n```\n", None);
    assert!(prompt.contains("Task: T") && prompt.contains("Files: a.rs, b.rs") && prompt.contains("- builds"));
    assert!(prompt.contains("FILE: a.rs") && !worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("already read"));
    assert!(worker_prompt(&MutisConfig::default(), &task, "", Some("it changed no file")).contains("previous attempt failed: it changed no file"));
}

#[test]
fn worker_prompt_requires_vietnamese_prose_even_with_english_instructions() {
    let task = Delegation {
        title: "T".into(),
        files: vec!["a.rs".into()],
        instructions: "change x".into(),
        acceptance: vec![],
        depends_on: vec![],
        hard: false,
        trivial: false,
    };
    let prompt = worker_prompt(&MutisConfig::default(), &task, "", None);
    // Applied at runtime, so a saved configuration without the rule still receives it.
    assert!(prompt.contains(crate::core::rule::mutis_high_delegation::VIETNAMESE_RULE));
    assert!(prompt.contains("user-facing word in Vietnamese") && prompt.contains("even when the task instructions are in English"));
    // The English task text and the internal instructions are untouched.
    assert!(prompt.contains("Instructions:\nchange x") && prompt.contains("Make the edits yourself with the file tools"));
}

#[test]
fn preload_keeps_small_plain_files_only() {
    let root = std::env::temp_dir().join(format!("ai-hub-preload-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("small.ts"), "const a = 1;\n").unwrap();
    std::fs::write(root.join("big.ts"), "x\n".repeat(PRELOAD_MAX_LINES + 1)).unwrap();
    std::fs::write(root.join("fenced.md"), "```\ncode\n```\n").unwrap();
    let files = ["small.ts", "big.ts", "fenced.md", "missing.ts"].map(String::from);
    let text = preload(root.to_str().unwrap(), &files);
    assert!(text.contains("FILE: small.ts\nconst a = 1;") && !text.contains("big.ts") && !text.contains("fenced.md") && !text.contains("missing"));
    assert_eq!(crate::providers::deepseek_agent::parse_edit_only(&text), vec![("small.ts".to_owned(), "const a = 1;".to_owned())]);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_check_keeps_one_identity_from_start_to_verdict() {
    let task = Delegation { title: " Fix Login ".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
    // The same check keeps its id in `check` and `announce_check`; a retry and another task never collide.
    assert_eq!(check_id(&task, 1), "fix login#1");
    assert_eq!(check_id(&task, 1), check_id(&task, 1));
    assert_ne!(check_id(&task, 1), check_id(&task, 2));
    let other = Delegation { title: "Fix Login #1".into(), ..task.clone() };
    assert_ne!(check_id(&task, 1), check_id(&other, 1));
}

#[test]
fn checker_activity_carries_tester_identity_and_never_a_chat_turn() {
    let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![], depends_on: vec![], hard: false, trivial: false };
    let event = checker_activity("a#1", "start", &task, 1, "gpt-5-codex", "");
    assert_eq!(event.name, "checker_activity");
    assert!(event.data.get("id").is_none());
    assert_eq!(event.data["checker_activity"]["id"], "a#1");
    assert_eq!(event.data["checker_activity"]["phase"], "start");
    assert_eq!(event.data["checker_activity"]["title"], "A");
    assert_eq!(event.data["checker_activity"]["attempt"], 1);
    assert_eq!(event.data["checker_activity"]["model"], "gpt-5-codex");
    assert_eq!(event.data["checker_activity"]["provider_name"], "Người kiểm tra");
    assert_eq!(event.data["checker_activity"]["text"], "");
    assert_eq!(checker_activity("a#1", "done", &task, 1, "gpt-5-codex", "").data["checker_activity"]["phase"], "done");
}

#[test]
fn checker_progress_keeps_tool_lines_and_drops_the_raw_verdict() {
    assert_eq!(checker_progress("  chạy cargo test  ").as_deref(), Some("chạy cargo test"));
    assert_eq!(checker_progress("{\"verdict\":\"PASS\"}"), None);
    assert_eq!(checker_progress("[tool] read"), None);
    assert_eq!(checker_progress("```json"), None);
    assert_eq!(checker_progress("   "), None);
    // 200 characters plus the "…" that `truncate_chars` appends when it cuts.
    assert_eq!(checker_progress(&"x".repeat(400)).map(|line| line.chars().count()), Some(201));
}
