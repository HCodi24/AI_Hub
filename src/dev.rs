use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

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
