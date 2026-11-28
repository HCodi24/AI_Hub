//! Per-file line counts of what an AI run changed, taken from git before and after the run.
use crate::core::process::git_output;
use serde::{Deserialize, Serialize};
use std::{collections::{BTreeMap, BTreeSet}, path::Path};

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

/// The git diff of the changed files, for sources that do not send their own (Claude CLI, Antigravity): the working tree against HEAD,
/// with each new untracked file as an addition. shortcut: it also holds edits made before the run in the same files, and is cut at
/// `MAX_DIFF` characters; keep per-run content snapshots if exact per-run diffs matter.
pub async fn diff_of(folder: &Path, files: &[FileChange]) -> String {
    const MAX_DIFF: usize = 200_000;
    let untracked: std::collections::BTreeSet<String> = match git_output(folder, &["-c", "core.quotepath=off", "ls-files", "--others", "--exclude-standard"]).await {
        Ok(out) => String::from_utf8_lossy(&out.stdout).lines().map(str::to_owned).collect(),
        Err(_) => return String::new(),
    };
    let tracked: Vec<&str> = files.iter().map(|f| f.path.as_str()).filter(|p| !untracked.contains(*p)).collect();
    let mut diff = String::new();
    if !tracked.is_empty() {
        let mut args = vec!["-c", "core.quotepath=off", "diff", "--no-color", "--no-renames", "HEAD", "--"];
        args.extend(tracked);
        if let Ok(out) = git_output(folder, &args).await {
            diff.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }
    for file in files.iter().filter(|f| untracked.contains(&f.path)) {
        // `--no-index` exits with 1 when the files differ; only its output matters.
        if let Ok(out) = git_output(folder, &["-c", "core.quotepath=off", "diff", "--no-color", "--no-index", "--", "/dev/null", &file.path]).await {
            diff.push_str(&String::from_utf8_lossy(&out.stdout));
        }
    }
    diff.chars().take(MAX_DIFF).collect()
}

/// Replace sections whose line counts disagree with the changes recorded for the run.
pub async fn reconcile_diff(folder: &Path, diff: &str, files: &[FileChange]) -> String {
    let (kept, mismatched) = reconcile_sections(diff, files);
    if mismatched.is_empty() {
        return kept;
    }
    kept + &diff_of(folder, &mismatched).await
}

fn reconcile_sections(diff: &str, files: &[FileChange]) -> (String, Vec<FileChange>) {
    let mut starts = diff.match_indices("diff --git ")
        .map(|(offset, _)| offset)
        .filter(|&offset| offset == 0 || diff.as_bytes()[offset - 1] == b'\n')
        .peekable();
    let prefix = &diff[..starts.peek().copied().unwrap_or(diff.len())];
    let mut sections = Vec::new();
    let mut counts = BTreeMap::<String, (usize, usize)>::new();
    while let Some(start) = starts.next() {
        let section = &diff[start..starts.peek().copied().unwrap_or(diff.len())];
        let path = section_path(section);
        if let Some(path) = &path {
            let (added, removed) = counts.entry(path.clone()).or_default();
            for line in section.lines() {
                if line.starts_with('+') && !line.starts_with("+++ ") {
                    *added += 1;
                } else if line.starts_with('-') && !line.starts_with("--- ") {
                    *removed += 1;
                }
            }
        }
        sections.push((path, section));
    }

    let mut mismatched_paths = BTreeSet::new();
    let mut mismatched = Vec::new();
    for file in files {
        let path = file.path.replace('\\', "/");
        if counts.get(&path).copied().unwrap_or_default() != (file.added, file.removed) {
            mismatched_paths.insert(path);
            mismatched.push(file.clone());
        }
    }
    if mismatched.is_empty() {
        return (diff.to_owned(), mismatched);
    }
    let mut kept = prefix.to_owned();
    for (path, section) in sections {
        if !path.is_some_and(|path| mismatched_paths.contains(&path)) {
            kept.push_str(section);
        }
    }
    (kept, mismatched)
}

fn section_path(section: &str) -> Option<String> {
    fn header_path(path: &str, prefix: &str) -> Option<String> {
        path.trim_end_matches('\t')
            .trim_matches('"')
            .replace('\\', "/")
            .strip_prefix(prefix)
            .map(str::to_owned)
    }

    let mut lines = section.lines();
    let header = lines.next()?.strip_prefix("diff --git ")?;
    let mut old_path = None;
    for line in lines.take_while(|line| !line.starts_with("@@")) {
        if let Some(path) = line.strip_prefix("--- ") {
            old_path = header_path(path, "a/");
        } else if let Some(path) = line.strip_prefix("+++ ") {
            if path == "/dev/null" && old_path.is_some() {
                return old_path;
            }
            if let Some(path) = header_path(path, "b/") {
                return Some(path);
            }
        }
    }
    let header = header.replace('\\', "/");
    let (_, path) = header.split_once("\" \"b/").or_else(|| header.split_once(" b/"))?;
    Some(path.trim_end_matches('"').to_owned())
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
mod tests;
