use super::git::clean_paths;
use super::*;
use std::{path::PathBuf, time::SystemTime};

#[test]
fn git_command_builders_use_fixed_arguments() {
    assert_eq!(
        git_checkpoint_args("2026-11-10T12:30:00+07:00"),
        vec![
            vec!["add".to_owned(), "-A".to_owned()],
            vec![
                "commit".to_owned(),
                "-m".to_owned(),
                "checkpoint: 2026-11-10T12:30:00+07:00".to_owned()
            ]
        ]
    );
    assert_eq!(
        git_commit_args("ship it"),
        vec![
            vec!["add".to_owned(), "-A".to_owned()],
            vec!["commit".to_owned(), "-m".to_owned(), "ship it".to_owned()]
        ]
    );
}

#[test]
fn clean_whitelist_is_only_src_and_web_src() {
    assert_eq!(clean_paths(), ["src", "web/src"]);
    assert_eq!(
        git_reset_args(),
        vec![
            vec!["reset".to_owned(), "--hard".to_owned(), "HEAD".to_owned()],
            vec![
                "clean".to_owned(),
                "-fd".to_owned(),
                "--".to_owned(),
                "src".to_owned()
            ],
            vec![
                "clean".to_owned(),
                "-fd".to_owned(),
                "--".to_owned(),
                "web/src".to_owned()
            ]
        ]
    );
}

#[test]
fn root_discovery_walks_up_and_checks_package_name() {
    let root = std::env::temp_dir().join(format!("ai-hub-root-{}", std::process::id()));
    let nested = root.join("a/b");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"other\"\n").unwrap();
    assert_eq!(discover_repo_root(&nested), None);
    std::fs::write(root.join("Cargo.toml"), "[package]\nname = \"ai-hub\"\n").unwrap();
    assert_eq!(discover_repo_root(&nested), Some(root.clone()));
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn mtime_detection_compares_snapshots_and_detects_removed_files() {
    let a = PathBuf::from("a.rs");
    let b = PathBuf::from("b.rs");
    let before = [(a.clone(), Some(SystemTime::UNIX_EPOCH)), (b.clone(), None)]
        .into_iter()
        .collect();
    let after = [(
        a.clone(),
        Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1)),
    )]
    .into_iter()
    .collect();
    assert_eq!(changed_files(&before, &after), vec![a, b]);
}
