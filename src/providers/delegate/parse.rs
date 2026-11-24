use super::Delegation;

const FENCE: &str = "```mutis-delegate";

/// Valid delegations in the text, plus a message for every block that did not parse.
pub fn extract(text: &str) -> (Vec<Delegation>, Vec<String>) {
    let (mut tasks, mut errors) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(start) = rest.find(FENCE) {
        let body = &rest[start + FENCE.len()..];
        let end = body.find("```").unwrap_or(body.len());
        match serde_json::from_str::<Delegation>(body[..end].trim()) {
            Ok(task) if !task.title.trim().is_empty() && !task.instructions.trim().is_empty() => tasks.push(task),
            Ok(_) => errors.push("a mutis-delegate block has an empty title or instructions".into()),
            Err(error) => errors.push(format!("a mutis-delegate block is not valid JSON ({error})")),
        }
        rest = &body[end..];
    }
    (tasks, errors)
}

/// Style and markup files by extension. Component files (.tsx, .jsx, .vue, .svelte) are NOT listed: the same file holds layout
/// changes and plain data wiring, so the coordinator judges those (see `NO_UI_NOTE`).
pub(super) fn is_ui_file(path: &str) -> bool {
    const UI: [&str; 5] = ["css", "scss", "sass", "less", "html"];
    path.rsplit_once('.').is_some_and(|(_, ext)| UI.iter().any(|ui| ext.eq_ignore_ascii_case(ui)))
}

/// With the "DeepSeek may do UI" switch off, a task with UI files never runs on DeepSeek: it goes back to the coordinator.
/// Returns the tasks to run and the notes for the coordinator.
pub(super) fn guard_ui(tasks: Vec<Delegation>, ui_allowed: bool) -> (Vec<Delegation>, Vec<String>) {
    if ui_allowed {
        return (tasks, Vec::new());
    }
    let (mut kept, mut notes, mut dropped) = (Vec::new(), Vec::new(), Vec::<String>::new());
    let key = |title: &str| title.trim().to_lowercase();
    for task in tasks {
        let blocked_by = task.depends_on.iter().find(|dep| dropped.contains(&key(dep))).cloned();
        if let Some(dep) = blocked_by {
            notes.push(format!("Task \"{}\" was not run: it depends on \"{dep}\", which you must implement yourself first (UI is off for DeepSeek). Resend it afterwards.", task.title.trim()));
            dropped.push(key(&task.title));
        } else if !task.hard && task.files.iter().any(|file| is_ui_file(file)) {
            notes.push(format!("Task \"{}\" has UI files and DeepSeek may not do UI (setting). Implement it yourself.", task.title.trim()));
            dropped.push(key(&task.title));
        } else {
            kept.push(task);
        }
    }
    (kept, notes)
}

/// Paths Codex named must be real before a worker run is spent: an existing file, or a new file in an existing folder.
pub fn check_files(folder: &str, files: &[String]) -> Vec<String> {
    if files.is_empty() {
        return vec!["the files list is empty: name the exact files".into()];
    }
    let root = std::path::Path::new(folder);
    files.iter().filter_map(|file| {
        let relative = file.replace('\\', "/");
        if relative.starts_with('/') || relative.as_bytes().get(1) == Some(&b':') || relative.split('/').any(|part| part == "..") {
            return Some(format!("`{file}` must be a relative path inside the project"));
        }
        let path = root.join(&relative);
        let parent_exists = path.parent().is_some_and(|parent| parent.is_dir());
        (!path.is_file() && !parent_exists).then(|| format!("`{file}` does not exist and neither does its folder"))
    }).collect()
}
