//! Per-file line counts of what an AI run changed, taken from git before and after the run.
use crate::core::process::git_output;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileChange {
    pub path: String,
    pub added: usize,
    pub removed: usize,
    #[serde(default = "modified")]
    pub status: String,
}

fn modified() -> String { "modified".into() }

// Counts and existence at the time of the snapshot.
pub type Snapshot = BTreeMap<String, (usize, usize, bool)>;

/// Uncommitted +/- lines per file (tracked and untracked). Empty outside a git repo.
/// shortcut: runs in the same folder at the same time are not told apart, split per chat if that matters.
pub async fn snapshot(folder: &Path) -> Snapshot {
    let mut map = Snapshot::new();
    let args = ["-c", "core.quotepath=off"];
    let tracked = git_output(folder, &[&args[..], &["diff", "--numstat", "--no-renames", "HEAD"]].concat()).await;
    let Ok(out) = tracked else { return map };
    if !out.status.success() {
        return map;
    }
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let mut part = line.splitn(3, '\t');
        if let (Some(a), Some(r), Some(path)) = (part.next(), part.next(), part.next()) {
            // Binary files report "-"; count them as 0 lines but still list them.
            map.insert(path.to_owned(), (a.parse().unwrap_or(0), r.parse().unwrap_or(0), folder.join(path).exists()));
        }
    }
    if let Ok(out) = git_output(folder, &[&args[..], &["ls-files", "--others", "--exclude-standard"]].concat()).await {
        for path in String::from_utf8_lossy(&out.stdout).lines() {
            let lines = std::fs::read_to_string(folder.join(path)).map_or(0, |text| text.lines().count());
            map.insert(path.to_owned(), (lines, 0, true));
        }
    }
    // Include unchanged tracked files so creation is distinguishable from editing.
    if let Ok(out) = git_output(folder, &[&args[..], &["ls-files"]].concat()).await {
        for path in String::from_utf8_lossy(&out.stdout).lines() {
            map.entry(path.to_owned()).or_insert((0, 0, folder.join(path).exists()));
        }
    }
    map
}

/// Files whose counts moved between the two snapshots, with the difference.
pub fn changes(before: &Snapshot, after: &Snapshot) -> Vec<FileChange> {
    before.keys().chain(after.keys()).collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .filter_map(|path| {
            let old = before.get(path).copied().unwrap_or((0, 0, false));
            let now = after.get(path).copied().unwrap_or((0, 0, false));
            if old == now { return None; }
            let status = match (old.2, now.2) {
                (false, true) => "added",
                (true, false) => "deleted",
                _ => "modified",
            };
            Some(FileChange {
                path: path.clone(), added: now.0.saturating_sub(old.0),
                removed: now.1.saturating_sub(old.1), status: status.into(),
            })
        }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
