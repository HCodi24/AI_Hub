use super::edit::{exact_replace, match_line_endings};
use super::path::safe_path;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

pub(in crate::providers::agent) fn run_tool(
    root: &Path,
    name: &str,
    args: &Value,
    max_lines: u32,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> String {
    run_file_tool(root, name, args, max_lines, snapshots)
        .unwrap_or_else(|error| format!("error: {error:#}"))
}

fn run_file_tool(
    root: &Path,
    name: &str,
    args: &Value,
    max_lines: u32,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> Result<String> {
    let raw = args["path"].as_str().context("Thiếu path")?;
    let path = safe_path(root, raw)?;
    let relative = path
        .strip_prefix(root)?
        .to_string_lossy()
        .replace('\\', "/");
    match name {
        "list_dir" => list_dir(&path),
        "read_file" => read_file(&path, args, max_lines),
        "edit_file" => edit_file(&path, args, relative, snapshots),
        "write_file" => write_file(&path, args, relative, snapshots),
        _ => bail!("Công cụ không tồn tại: {name}"),
    }
}

fn list_dir(path: &Path) -> Result<String> {
    let mut entries = fs::read_dir(path)?
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

fn read_file(path: &Path, args: &Value, max_lines: u32) -> Result<String> {
    let text = fs::read_to_string(path)?;
    let lines = text.lines().collect::<Vec<_>>();
    let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
    let end = args["end_line"].as_u64().unwrap_or(lines.len() as u64) as usize;
    // A long file read without a range used to be an error that cost a whole extra step: give the first part and say how to go on.
    let long = args["start_line"].is_null()
        && args["end_line"].is_null()
        && lines.len() > max_lines as usize;
    if end < start {
        bail!("end_line phải >= start_line");
    }
    let end = end.min(lines.len()).min(start.saturating_add(399));
    let end = if long { end.min(max_lines as usize) } else { end };
    let mut shown = lines
        .iter()
        .enumerate()
        .skip(start - 1)
        .take(end.saturating_sub(start) + 1)
        .map(|(index, line)| format!("{}: {}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    if long {
        shown.push_str(&format!(
            "\n[file has {} lines, showing 1-{end}; call again with start_line={} for the rest]",
            lines.len(),
            end + 1
        ));
    }
    Ok(shown)
}

fn edit_file(
    path: &Path,
    args: &Value,
    relative: String,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> Result<String> {
    let text = fs::read_to_string(path)?;
    let old = args["old_text"].as_str().context("Thiếu old_text")?;
    let new = args["new_text"].as_str().context("Thiếu new_text")?;
    let (old, new) = (match_line_endings(&text, old), match_line_endings(&text, new));
    let updated = exact_replace(&text, &old, &new)?;
    snapshots.entry(relative).or_insert(Some(text));
    fs::write(path, updated)?;
    Ok("edited".into())
}

fn write_file(
    path: &Path,
    args: &Value,
    relative: String,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> Result<String> {
    let content = args["content"].as_str().context("Thiếu content")?;
    snapshots
        .entry(relative)
        .or_insert_with(|| fs::read_to_string(path).ok());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    // Keep CRLF files CRLF: the model writes LF, which would make every line look changed.
    let content = match fs::read_to_string(path) {
        Ok(existing) => match_line_endings(&existing, content),
        Err(_) => content.to_owned(),
    };
    fs::write(path, &content)?;
    Ok(format!("wrote {} chars", content.chars().count()))
}
