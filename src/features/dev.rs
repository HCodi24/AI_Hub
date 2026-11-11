use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

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
    ["src", "ui"]
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

pub fn iso_now() -> String {
    let seconds = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (seconds / 86_400) as i64;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let rem = seconds % 86_400;
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem / 60 % 60,
        rem % 60
    )
}

#[cfg(test)]
mod command_tests {
    use super::*;

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
    fn clean_whitelist_is_only_src_and_ui() {
        assert_eq!(clean_paths(), ["src", "ui"]);
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
                    "ui".to_owned()
                ]
            ]
        );
    }
}

pub type Snapshot = BTreeMap<PathBuf, Option<SystemTime>>;

pub fn discover_repo_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| {
            let manifest = dir.join("Cargo.toml");
            std::fs::read_to_string(manifest)
                .ok()
                .is_some_and(|text| text.lines().any(|line| line.trim() == "name = \"ai-hub\""))
        })
        .map(Path::to_path_buf)
}

pub fn changed_files(before: &Snapshot, after: &Snapshot) -> Vec<PathBuf> {
    after
        .iter()
        .filter(|(path, stamp)| before.get(*path) != Some(*stamp))
        .map(|(path, _)| path.clone())
        .chain(
            before
                .keys()
                .filter(|path| !after.contains_key(*path))
                .cloned(),
        )
        .collect()
}

pub fn snapshot(repo: &Path) -> (Snapshot, Snapshot) {
    let mut ui = Snapshot::new();
    let mut rust = Snapshot::new();
    collect_files(&repo.join("ui"), &mut ui, |_| true);
    collect_files(&repo.join("src"), &mut rust, |p| {
        p.extension().is_some_and(|x| x == "rs")
    });
    for name in ["Cargo.toml", "Cargo.lock"] {
        let path = repo.join(name);
        if path.is_file() {
            rust.insert(path, stamp(&repo.join(name)));
        }
    }
    (ui, rust)
}

/// The exported React UI (web/out). `next build` empties it first, so the watcher waits until it is complete and
/// stable before telling the page to reload.
pub fn snapshot_web_out(repo: &Path) -> Snapshot {
    let mut out = Snapshot::new();
    collect_files(&repo.join("web/out"), &mut out, |_| true);
    out
}

/// Source files of the React UI (web/src + its config). They need `npm run build` before the page can show them.
pub fn snapshot_web_src(repo: &Path) -> Snapshot {
    let mut web = Snapshot::new();
    collect_files(&repo.join("web/src"), &mut web, |_| true);
    for name in ["package.json", "next.config.js", "tsconfig.json"] {
        let path = repo.join("web").join(name);
        if path.is_file() {
            web.insert(path.clone(), stamp(&path));
        }
    }
    web
}

fn collect_files(root: &Path, out: &mut Snapshot, include: impl Fn(&Path) -> bool + Copy) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_files(&path, out, include);
        } else if path.is_file() && include(&path) {
            out.insert(path.clone(), stamp(&path));
        }
    }
}

fn stamp(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
