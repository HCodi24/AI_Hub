fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_lowercase()
}

/// With a scope, edits are limited to the listed files; `write_file` may also create a NEW file in a listed file's folder.
pub(super) fn in_scope(root: &std::path::Path, scope: &[String], path: &str, creating: bool) -> bool {
    let path = normalize(path);
    let folder = |p: &str| p.rsplit_once('/').map_or(String::new(), |(dir, _)| dir.to_owned());
    scope.iter().map(|entry| normalize(entry)).any(|entry| {
        entry == path || (creating && !root.join(&path).exists() && folder(&entry) == folder(&path))
    })
}
