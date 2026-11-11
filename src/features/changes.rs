//! Per-file line counts of what an AI run changed, taken from git before and after the run.
use crate::core::process::git_output;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct FileChange {
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

pub type Snapshot = BTreeMap<String, (usize, usize)>;

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
            map.insert(path.to_owned(), (a.parse().unwrap_or(0), r.parse().unwrap_or(0)));
        }
    }
    if let Ok(out) = git_output(folder, &[&args[..], &["ls-files", "--others", "--exclude-standard"]].concat()).await {
        for path in String::from_utf8_lossy(&out.stdout).lines() {
            let lines = std::fs::read_to_string(folder.join(path)).map_or(0, |text| text.lines().count());
            map.insert(path.to_owned(), (lines, 0));
        }
    }
    map
}

/// Files whose counts moved between the two snapshots, with the difference.
pub fn changes(before: &Snapshot, after: &Snapshot) -> Vec<FileChange> {
    after
        .iter()
        .filter(|(path, now)| before.get(*path) != Some(*now))
        .map(|(path, (a, r))| {
            let (ba, br) = before.get(path).copied().unwrap_or((0, 0));
            FileChange { path: path.clone(), added: a.saturating_sub(ba), removed: r.saturating_sub(br) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_only_files_that_changed_during_the_run() {
        let before = Snapshot::from([("a.rs".into(), (2, 1)), ("b.rs".into(), (4, 0))]);
        let after = Snapshot::from([("a.rs".into(), (5, 1)), ("b.rs".into(), (4, 0)), ("c.rs".into(), (7, 0))]);
        assert_eq!(
            changes(&before, &after),
            vec![
                FileChange { path: "a.rs".into(), added: 3, removed: 0 },
                FileChange { path: "c.rs".into(), added: 7, removed: 0 },
            ]
        );
    }
}
