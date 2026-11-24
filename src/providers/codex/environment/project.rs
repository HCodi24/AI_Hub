use super::project_root;
use crate::config::CodexConfig;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Explicitly load only the requested sources, with canonical path checks and bounded size.
pub fn project_context(folder: &Path, settings: &CodexConfig) -> Result<String> {
    if !settings.project_skills && !settings.project_instructions {
        return Ok(String::new());
    }
    let folder = std::fs::canonicalize(folder)?;
    let root = project_root(&folder);
    let mut dirs = folder
        .ancestors()
        .take_while(|p| p.starts_with(&root))
        .collect::<Vec<_>>();
    dirs.reverse();
    let mut files = Vec::new();
    for dir in dirs {
        if settings.project_instructions {
            let override_file = dir.join("AGENTS.override.md");
            files.push(if override_file.is_file() {
                override_file
            } else {
                dir.join("AGENTS.md")
            });
        }
        if settings.project_skills {
            for source in [dir.join(".agents/skills"), dir.join(".codex/skills")] {
                collect_skills(&source, &mut files, 0)?;
            }
        }
    }
    files.dedup();
    let mut context = String::new();
    for path in files {
        if !path.is_file() {
            continue;
        }
        let path = std::fs::canonicalize(path)?;
        if !path.starts_with(&root) {
            bail!("Nguồn hướng dẫn dự án trỏ ra ngoài thư mục dự án");
        }
        if std::fs::metadata(&path)?.len() > 32000 {
            bail!("File hướng dẫn dự án vượt giới hạn 32 KB");
        }
        let text = std::fs::read_to_string(&path).context("File hướng dẫn phải dùng UTF-8")?;
        if context.len() + text.len() > 64000 {
            bail!("Tổng hướng dẫn dự án vượt giới hạn 64 KB");
        }
        context.push_str(&format!("\nProject source: {}\n{}\n", path.display(), text));
    }
    Ok(context)
}

fn collect_skills(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if depth > 4 || files.len() >= 64 {
        bail!("Quá nhiều skill hoặc thư mục skill quá sâu");
    }
    let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect_skills(&entry.path(), files, depth + 1)?;
        } else if entry.file_name() == "SKILL.md" {
            if files.len() >= 64 {
                bail!("Quá nhiều file skill trong dự án");
            }
            files.push(entry.path());
        }
    }
    Ok(())
}
