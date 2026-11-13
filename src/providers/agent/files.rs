use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

pub(super) fn anthropic_definitions(write: bool) -> Vec<Value> {
    let s = json!({"type":"string"});
    let all = vec![
        json!({
            "name": "list_dir",
            "description": "List up to 300 entries in a relative directory.",
            "input_schema": {"type":"object","properties":{"path":s},"required":["path"]}
        }),
        json!({
            "name": "read_file",
            "description": "Read numbered lines from a text file. At most 400 lines; use ranges for long files.",
            "input_schema": {
                "type":"object",
                "properties":{"path":s,"start_line":{"type":"integer"},"end_line":{"type":"integer"}},
                "required":["path"]
            }
        }),
        json!({
            "name": "edit_file",
            "description": "Replace one exact unique occurrence.",
            "input_schema": {
                "type":"object",
                "properties":{"path":s,"old_text":s,"new_text":s},
                "required":["path","old_text","new_text"]
            }
        }),
        json!({
            "name": "write_file",
            "description": "Create or overwrite a UTF-8 file.",
            "input_schema": {"type":"object","properties":{"path":s,"content":s},"required":["path","content"]}
        }),
    ];
    if write {
        all
    } else {
        all.into_iter()
            .filter(|tool| matches!(tool["name"].as_str(), Some("list_dir" | "read_file")))
            .collect()
    }
}

pub(super) fn openai_definitions(write: bool) -> Vec<Value> {
    anthropic_definitions(write)
        .into_iter()
        .map(|tool| {
            json!({"type":"function","function":{
                "name":tool["name"],
                "description":tool["description"],
                "parameters":tool["input_schema"]
            }})
        })
        .collect()
}

pub(super) fn safe_path(root: &Path, raw: &str) -> Result<PathBuf> {
    if raw.starts_with('/') || raw.starts_with('\\') || raw.as_bytes().get(1) == Some(&b':') {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let normalized = raw.replace('\\', "/");
    if normalized.split('/').any(|part| part == "..") {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let rel = Path::new(&normalized);
    if rel.as_os_str().is_empty()
        || rel.is_absolute()
        || rel
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    if rel.components().any(|component| {
        [".git", "target", "node_modules", ".codegraph"]
            .iter()
            .any(|blocked| {
                component
                    .as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(blocked)
            })
    }) {
        bail!("Đường dẫn thuộc vùng bị cấm");
    }
    let candidate = root.join(rel);
    let mut ancestor = candidate.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .context("Đường dẫn không hợp lệ")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().context("Đường dẫn không hợp lệ")?;
    }
    let mut check = ancestor.canonicalize()?;
    for part in suffix.iter().rev() {
        check.push(part);
    }
    if check != root && !check.starts_with(root) {
        bail!("Đường dẫn vượt khỏi thư mục chat");
    }
    Ok(candidate)
}

pub(super) fn run_tool(
    root: &Path,
    name: &str,
    args: &Value,
    max_lines: u32,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> String {
    let result = (|| -> Result<String> {
        let raw = args["path"].as_str().context("Thiếu path")?;
        let path = safe_path(root, raw)?;
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        match name {
            "list_dir" => {
                let mut entries = fs::read_dir(&path)?
                    .filter_map(Result::ok)
                    .map(|entry| {
                        let kind = if entry
                            .file_type()
                            .map(|file_type| file_type.is_dir())
                            .unwrap_or(false)
                        {
                            "d"
                        } else {
                            "f"
                        };
                        format!("{kind} {}", entry.file_name().to_string_lossy())
                    })
                    .collect::<Vec<_>>();
                entries.sort();
                entries.truncate(300);
                Ok(entries.join("\n"))
            }
            "read_file" => {
                let text = fs::read_to_string(&path)?;
                let lines = text.lines().collect::<Vec<_>>();
                let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
                let end = args["end_line"].as_u64().unwrap_or(lines.len() as u64) as usize;
                if args["start_line"].is_null()
                    && args["end_line"].is_null()
                    && lines.len() > max_lines as usize
                {
                    bail!(
                        "File dài {} dòng, hãy đọc theo khoảng start_line/end_line (tối đa 400 dòng mỗi lần)",
                        lines.len()
                    );
                }
                if end < start {
                    bail!("end_line phải >= start_line");
                }
                let end = end.min(lines.len()).min(start.saturating_add(399));
                Ok(lines
                    .iter()
                    .enumerate()
                    .skip(start - 1)
                    .take(end.saturating_sub(start) + 1)
                    .map(|(index, line)| format!("{}: {}", index + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            "edit_file" => {
                let text = fs::read_to_string(&path)?;
                let old = args["old_text"].as_str().context("Thiếu old_text")?;
                let new = args["new_text"].as_str().context("Thiếu new_text")?;
                let (old, new) = (match_line_endings(&text, old), match_line_endings(&text, new));
                let updated = exact_replace(&text, &old, &new)?;
                snapshots.entry(relative).or_insert(Some(text));
                fs::write(&path, updated)?;
                Ok("edited".into())
            }
            "write_file" => {
                let content = args["content"].as_str().context("Thiếu content")?;
                snapshots
                    .entry(relative)
                    .or_insert_with(|| fs::read_to_string(&path).ok());
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                // Keep CRLF files CRLF: the model writes LF, which would make every line look changed.
                let content = match fs::read_to_string(&path) {
                    Ok(existing) => match_line_endings(&existing, content),
                    Err(_) => content.to_owned(),
                };
                fs::write(&path, &content)?;
                Ok(format!("wrote {} chars", content.chars().count()))
            }
            _ => bail!("Công cụ không tồn tại: {name}"),
        }
    })();
    result.unwrap_or_else(|error| format!("error: {error:#}"))
}

/// The model always sends LF; if the file uses CRLF, convert its text so multi-line matches and writes keep the file's endings.
pub(super) fn match_line_endings(file: &str, text: &str) -> String {
    if file.contains("\r\n") {
        text.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        text.to_owned()
    }
}

pub(super) fn exact_replace(text: &str, old: &str, new: &str) -> Result<String> {
    let count = text.matches(old).count();
    if count != 1 {
        bail!("old_text khớp {count} lần; cần đúng 1 lần");
    }
    Ok(text.replacen(old, new, 1))
}

pub(super) fn line_counts(before: &str, after: &str) -> (usize, usize) {
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

pub(super) fn make_diff(root: &Path, snapshots: &BTreeMap<String, Option<String>>) -> String {
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

#[cfg(test)]
mod tests {
    #[test]
    fn crlf_files_accept_lf_edits_and_stay_crlf() {
        let file = "a
b
c
";
        let (old, new) = (match_line_endings(file, "a
b"), match_line_endings(file, "a
x"));
        assert_eq!(exact_replace(file, &old, &new).unwrap(), "a
x
c
");
        assert_eq!(match_line_endings("a
b
", "x
y"), "x
y");
    }

    use super::*;

    #[test]
    fn path_is_confined_and_blocks_generated_directories() {
        let root = std::env::current_dir().unwrap();
        for path in [
            "../escape",
            "C:\\temp\\x",
            ".git/config",
            "target/out",
            "node_modules/x",
            ".codegraph/index",
        ] {
            assert!(safe_path(&root, path).is_err(), "{path}");
        }
    }

    #[test]
    fn edit_requires_exactly_one_match_and_diff_counts_changes() {
        assert_eq!(
            exact_replace("a b a", "a", "x").unwrap_err().to_string(),
            "old_text khớp 2 lần; cần đúng 1 lần"
        );
        assert_eq!(exact_replace("a b", "a", "x").unwrap(), "x b");
        assert_eq!(line_counts("a\nb\n", "a\nc\n"), (1, 1));
    }

    #[test]
    fn tool_names_are_valid_for_openai_and_anthropic() {
        for write in [false, true] {
            for name in anthropic_definitions(write)
                .iter()
                .filter_map(|tool| tool["name"].as_str())
            {
                assert!(valid_tool_name(name), "{name}");
            }
            for tool in openai_definitions(write) {
                assert!(valid_tool_name(tool["function"]["name"].as_str().unwrap()));
            }
        }
    }

    fn valid_tool_name(name: &str) -> bool {
        !name.is_empty()
            && name.len() <= 64
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '-')
            })
    }
}
