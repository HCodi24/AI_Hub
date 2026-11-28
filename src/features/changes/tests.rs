use super::*;

fn file_change(path: &str, added: usize, removed: usize) -> FileChange {
    FileChange { path: path.into(), added, removed, status: modified() }
}

#[test]
fn reconcile_keeps_matching_sections_byte_for_byte() {
    let diff = "preamble\r\ndiff --git a/sub/a b/sub/a\r\n--- a/sub/a\r\n+++ b/sub/a\r\n@@ -1 +1 @@\r\n-old\r\n+new\r\ndiff --git a/sub/a b/sub/a\n--- a/sub/a\n+++ b/sub/a\n@@ -1 +1,2 @@\n new\n+more";
    let (kept, mismatched) = reconcile_sections(diff, &[file_change("sub\\a", 2, 1)]);
    assert_eq!(kept, diff);
    assert!(mismatched.is_empty());
}

#[test]
fn reconcile_removes_partial_sections_and_keeps_other_text() {
    let partial = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -0,0 +1 @@\n+partial\n";
    let other = "diff --git a/other b/other\n--- a/other\n+++ /dev/null\n@@ -1 +0,0 @@\n-line\n";
    let diff = format!("preamble\n{partial}{other}");
    let file = file_change("a", 3, 0);
    let (kept, mismatched) = reconcile_sections(&diff, std::slice::from_ref(&file));
    assert_eq!(kept, format!("preamble\n{other}"));
    assert_eq!(mismatched, vec![file]);
}

#[test]
fn reconcile_excludes_headers_from_counts() {
    let diff = "diff --git a/a b/a\n--- a/a\n+++ b/a\n";
    let (kept, mismatched) = reconcile_sections(diff, &[file_change("a", 0, 0)]);
    assert_eq!(kept, diff);
    assert!(mismatched.is_empty());
}

#[test]
fn reconcile_marks_missing_sections_only_for_nonzero_counts() {
    let diff = "preamble without sections";
    let file = file_change("missing", 2, 0);
    let (kept, mismatched) = reconcile_sections(diff, &[file.clone(), file_change("binary", 0, 0)]);
    assert_eq!(kept, diff);
    assert_eq!(mismatched, vec![file]);
}

#[test]
fn distinguishes_creation_deletion_and_line_only_edits() {
    let before = Snapshot::from([
        ("deleted".into(), (0, 0, true)),
        ("edited".into(), (0, 0, true)),
        ("untracked".into(), (3, 0, true)),
    ]);
    let after = Snapshot::from([
        ("deleted".into(), (0, 4, false)),
        ("edited".into(), (2, 0, true)),
        ("new-empty".into(), (0, 0, true)),
    ]);
    let result = changes(&before, &after);
    assert_eq!(result.iter().map(|f| (f.path.as_str(), f.status.as_str())).collect::<Vec<_>>(),
        vec![("deleted", "deleted"), ("edited", "modified"), ("new-empty", "added"), ("untracked", "deleted")]);
    let legacy: FileChange = serde_json::from_str(r#"{"path":"old","added":1,"removed":0}"#).unwrap();
    assert_eq!(legacy.status, "modified");
}

#[tokio::test]
async fn diff_of_covers_edited_new_and_untracked_files() {
    let dir = std::env::temp_dir().join(format!("changes-diff-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = |args: &[&str]| { std::process::Command::new("git").args(args).current_dir(&dir).output().unwrap(); };
    git(&["init", "-q"]);
    std::fs::write(dir.join("a.txt"), "one\n").unwrap();
    git(&["add", "a.txt"]);
    git(&["-c", "user.email=a@b", "-c", "user.name=t", "commit", "-qm", "base"]);
    std::fs::write(dir.join("a.txt"), "two\n").unwrap();
    std::fs::write(dir.join("b.txt"), "new\n").unwrap();
    let files = vec![
        FileChange { path: "a.txt".into(), added: 1, removed: 1, status: "modified".into() },
        FileChange { path: "b.txt".into(), added: 1, removed: 0, status: "added".into() },
    ];
    let diff = diff_of(&dir, &files).await;
    assert!(diff.contains("-one") && diff.contains("+two"), "{diff}");
    assert!(diff.contains("+++ b/b.txt") && diff.contains("+new"), "{diff}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn reports_only_files_that_changed_during_the_run() {
    let before = Snapshot::from([("a.rs".into(), (2, 1, true)), ("b.rs".into(), (4, 0, true))]);
    let after = Snapshot::from([("a.rs".into(), (5, 1, true)), ("b.rs".into(), (4, 0, true)), ("c.rs".into(), (7, 0, true))]);
    assert_eq!(
        changes(&before, &after),
        vec![
            FileChange { path: "a.rs".into(), added: 3, removed: 0, status: "modified".into() },
            FileChange { path: "c.rs".into(), added: 7, removed: 0, status: "added".into() },
        ]
    );
}
