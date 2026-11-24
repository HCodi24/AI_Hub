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

/// Paths named by the lead must refer to an existing file or a new file in an existing folder.
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
