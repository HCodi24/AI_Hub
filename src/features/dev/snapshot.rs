use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub type MtimeSnapshot = BTreeMap<PathBuf, Option<SystemTime>>;

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

pub fn changed_files(before: &MtimeSnapshot, after: &MtimeSnapshot) -> Vec<PathBuf> {
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

pub fn snapshot(repo: &Path) -> MtimeSnapshot {
    let mut rust = MtimeSnapshot::new();
    collect_files(&repo.join("src"), &mut rust, |p| {
        p.extension().is_some_and(|x| x == "rs")
    });
    for name in ["Cargo.toml", "Cargo.lock"] {
        let path = repo.join(name);
        if path.is_file() {
            rust.insert(path, stamp(&repo.join(name)));
        }
    }
    rust
}

/// The exported React UI (web/out). `next build` empties it first, so the watcher waits until it is complete and
/// stable before telling the page to reload.
pub fn snapshot_web_out(repo: &Path) -> MtimeSnapshot {
    let mut out = MtimeSnapshot::new();
    collect_files(&repo.join("web/out"), &mut out, |_| true);
    out
}

/// Source files of the React UI (web/src + its config). They need `npm run build` before the page can show them.
pub fn snapshot_web_src(repo: &Path) -> MtimeSnapshot {
    let mut web = MtimeSnapshot::new();
    collect_files(&repo.join("web/src"), &mut web, |_| true);
    for name in ["package.json", "next.config.js", "tsconfig.json"] {
        let path = repo.join("web").join(name);
        if path.is_file() {
            web.insert(path.clone(), stamp(&path));
        }
    }
    web
}

fn collect_files(root: &Path, out: &mut MtimeSnapshot, include: impl Fn(&Path) -> bool + Copy) {
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
