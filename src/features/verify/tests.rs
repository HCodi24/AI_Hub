use super::{
    detect::{command_available, detect},
    review::{may_create_fix, next_action, next_review_action, ReviewAction, ReviewTaskState},
    scan::{changed_files, question_pattern, scan_invalid_utf8, scan_mojibake},
    *,
};
use tokio::process::Command;
#[test]
fn saved_command_wins_over_global_and_blank_falls_back() {
    let mut mutis = crate::config::MutisConfig::default();
    mutis.verify_command = "global".into();
    let folder = Path::new("C:/Work/App/");
    assert_eq!(command_for(&mutis, folder), "global");
    mutis.verify_commands.insert(folder_key(folder), "npm test".into());
    assert_eq!(command_for(&mutis, Path::new("C:\\Work\\App")), "npm test", "slashes and a trailing slash do not matter");
    mutis.verify_commands.insert(folder_key(folder), "  ".into());
    assert_eq!(command_for(&mutis, folder), "global");
}
#[test]
fn a_folder_without_commands_is_unverified_not_passed() {
    let root = std::env::temp_dir().join(format!("ai-hub-unverified-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(64);
    let report = tokio::runtime::Runtime::new().unwrap().block_on(run(&root, "", &[], tx));
    assert!(report.unverified && report.ok, "nothing failed, but nothing was proven either");
    assert!(report.summary_lines[0].contains("CHƯA KIỂM TRA") && !report.summary_lines[0].contains("ĐẠT"));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn auto_detect_prefers_script_over_cargo() {
    let root = std::env::temp_dir().join(format!("ai-hub-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("scripts/verify")).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    std::fs::write(root.join("scripts/verify/verify.sh"), "true").unwrap();
    std::fs::write(root.join("scripts/verify.ps1"), "exit 0").unwrap();
    if command_available("powershell") {
        assert!(detect(&root)[0].1.contains("scripts/verify.ps1"));
    }
    let _ = std::fs::remove_file(root.join("scripts/verify.ps1"));
    assert_eq!(detect(&root)[0].0, "verify_script");
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn auto_detect_cargo_commands_are_ordered() {
    let root = std::env::temp_dir().join(format!("ai-hub-cargo-verify-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
    let commands = detect(&root);
    assert_eq!(commands[0].1, "cargo build --offline");
    assert_eq!(commands[1].1, "cargo test --offline");
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn mojibake_question_detector() {
    let broken_word = ["l", "?", "i"].concat();
    assert!(question_pattern(&broken_word));
    assert!(!question_pattern("const x = a?.b ?? c;"));
    assert!(!question_pattern("hello world"));
}
#[test]
fn fix_round_accounting_stops_at_limit() {
    assert!(may_create_fix("FAIL", true, 0, 2));
    assert!(may_create_fix("FAIL", true, 1, 2));
    assert!(!may_create_fix("FAIL", true, 2, 2));
    assert!(!may_create_fix("PASS", true, 0, 2));
    assert!(!may_create_fix("FAIL", false, 0, 2));
}
#[test]
fn review_transition_skips_without_consuming_a_round() {
    assert_eq!(
        next_action(false, "FAIL", true, 0, 2),
        ReviewAction::CreateFix
    );
    assert_eq!(next_action(false, "FAIL", true, 2, 2), ReviewAction::Fail);
    assert_eq!(
        next_review_action(&ReviewTaskState { verify_ok: true }, "SKIPPED", true, 0, 2),
        ReviewAction::Skip
    );
    assert_eq!(
        next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", true, 0, 2),
        ReviewAction::CreateFix
    );
    assert_eq!(
        next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", true, 2, 2),
        ReviewAction::Fail
    );
    assert_eq!(
        next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", false, 0, 2),
        ReviewAction::Fail
    );
    assert_eq!(
        next_review_action(&ReviewTaskState { verify_ok: true }, "PASS", true, 0, 2),
        ReviewAction::Complete
    );
    assert_eq!(
        next_review_action(
            &ReviewTaskState { verify_ok: false },
            "SKIPPED",
            false,
            0,
            2
        ),
        ReviewAction::Fail
    );
}
#[test]
fn invalid_encoding_is_reported_and_ignore_applies_only_to_mojibake() {
    let root = std::env::temp_dir().join(format!("ai-hub-utf8-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join(".tasks")).unwrap();
    let bad = root.join("hello.txt");
    std::fs::write(&bad, [b'x', 0xe0, b'y']).unwrap();
    let quoted = root.join(".tasks/quoted.md");
    std::fs::write(&quoted, ["l", "?", "i"].concat()).unwrap();
    assert_eq!(
        scan_invalid_utf8(&root, std::slice::from_ref(&bad)),
        vec!["hello.txt"]
    );
    assert!(scan_mojibake(&root, &[quoted], &[".tasks/**".into()]).is_empty());
    std::fs::remove_dir_all(root).unwrap();
}
#[tokio::test]
async fn changed_files_includes_untracked_git_files() {
    let root = std::env::temp_dir().join(format!("ai-hub-untracked-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let _ = Command::new("git")
        .args(["init", "-q"])
        .current_dir(&root)
        .status()
        .await
        .unwrap();
    std::fs::write(root.join("base.txt"), "base").unwrap();
    let _ = Command::new("git")
        .args(["add", "base.txt"])
        .current_dir(&root)
        .status()
        .await
        .unwrap();
    let _ = Command::new("git")
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "base",
            "-q",
        ])
        .current_dir(&root)
        .status()
        .await
        .unwrap();
    let new_file = root.join("hello.txt");
    std::fs::write(&new_file, "xin chào").unwrap();
    assert!(changed_files(&root).await.contains(&new_file));
    std::fs::remove_dir_all(root).unwrap();
}
