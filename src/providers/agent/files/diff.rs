use std::{collections::BTreeMap, fs, path::Path};

pub(in crate::providers::agent) fn line_counts(before: &str, after: &str) -> (usize, usize) {
    fn frequencies(text: &str) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for line in text.lines() {
            *counts.entry(line).or_insert(0) += 1;
        }
        counts
    }
    let old = frequencies(before);
    let new = frequencies(after);
    let added = new
        .iter()
        .map(|(line, count)| count.saturating_sub(old.get(line).copied().unwrap_or(0)))
        .sum();
    let removed = old
        .iter()
        .map(|(line, count)| count.saturating_sub(new.get(line).copied().unwrap_or(0)))
        .sum();
    (added, removed)
}

pub(in crate::providers::agent) fn make_diff(root: &Path, snapshots: &BTreeMap<String, Option<String>>) -> String {
    let mut out = String::new();
    for (path, before) in snapshots {
        let after = fs::read_to_string(root.join(path)).unwrap_or_default();
        out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
        let old = before.as_deref().unwrap_or("").lines().collect::<Vec<_>>();
        let new = after.lines().collect::<Vec<_>>();
        out.push_str(&format!("@@ -1,{} +1,{} @@\n", old.len(), new.len()));
        for line in &old {
            out.push('-');
            out.push_str(line);
            out.push('\n');
        }
        for line in &new {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        if out.len() > 20 * 1024 {
            out.truncate(20 * 1024);
            out.push_str("\n...[diff truncated at 20 KB]\n");
            break;
        }
    }
    out
}
