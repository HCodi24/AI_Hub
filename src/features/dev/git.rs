pub fn git_checkpoint_args(timestamp: &str) -> Vec<Vec<String>> {
    vec![
        vec!["add".into(), "-A".into()],
        vec![
            "commit".into(),
            "-m".into(),
            format!("checkpoint: {timestamp}"),
        ],
    ]
}

pub fn git_commit_args(message: &str) -> Vec<Vec<String>> {
    vec![
        vec!["add".into(), "-A".into()],
        vec!["commit".into(), "-m".into(), message.into()],
    ]
}

pub fn clean_paths() -> [&'static str; 2] {
    ["src", "web/src"]
}

pub fn git_reset_args() -> Vec<Vec<String>> {
    let mut args = vec![vec!["reset".into(), "--hard".into(), "HEAD".into()]];
    args.extend(
        clean_paths()
            .into_iter()
            .map(|path| vec!["clean".into(), "-fd".into(), "--".into(), path.into()]),
    );
    args
}
