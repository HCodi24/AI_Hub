//! App-controlled project context. Native discovery stays disabled for both switches.
use crate::config::ClaudeConfig;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn project_context(folder: &Path, settings: &ClaudeConfig) -> Result<String> {
    if !settings.project_skills && !settings.project_instructions {
        return Ok(String::new());
    }
    let folder = std::fs::canonicalize(folder)?;
    let root = folder
        .ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(&folder);
    let mut dirs = folder
        .ancestors()
        .take_while(|p| p.starts_with(root))
        .collect::<Vec<_>>();
    dirs.reverse();
    let mut files = Vec::new();
    for dir in dirs {
        if settings.project_instructions {
            for name in ["CLAUDE.md", "CLAUDE.local.md", ".claude/CLAUDE.md"] {
                if dir.join(name).is_file() {
                    files.push(dir.join(name));
                }
            }
            collect(&dir.join(".claude/rules"), root, &mut files, 0, false)?;
        }
        if settings.project_skills {
            for name in [".claude/skills", ".agents/skills"] {
                collect(&dir.join(name), root, &mut files, 0, true)?;
            }
        }
    }
    let mut context = String::new();
    for path in files {
        let path = std::fs::canonicalize(path)?;
        if !path.starts_with(root) {
            bail!("Nguồn Claude trỏ ra ngoài dự án");
        }
        if std::fs::metadata(&path)?.len() > 32_000 {
            bail!("File hướng dẫn Claude vượt 32 KB");
        }
        let text = std::fs::read_to_string(&path).context("Hướng dẫn Claude phải dùng UTF-8")?;
        let block = format!("\nProject source: {}\n{text}\n", path.display());
        if context.len() + block.len() > 64_000 {
            bail!("Tổng hướng dẫn Claude vượt 64 KB");
        }
        context.push_str(&block);
    }
    Ok(context)
}

fn collect(
    dir: &Path,
    root: &Path,
    files: &mut Vec<PathBuf>,
    depth: usize,
    skills: bool,
) -> Result<()> {
    if !dir.is_dir() {
        return Ok(());
    }
    if depth > 4 || files.len() >= 64 {
        bail!("Quá nhiều nguồn Claude hoặc thư mục quá sâu");
    }
    if !std::fs::canonicalize(dir)?.starts_with(root) {
        bail!("Nguồn Claude trỏ ra ngoài dự án");
    }
    let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            collect(&entry.path(), root, files, depth + 1, skills)?;
        } else if (skills && entry.file_name() == "SKILL.md")
            || (!skills && entry.path().extension().is_some_and(|e| e == "md"))
        {
            if files.len() >= 64 {
                bail!("Quá nhiều nguồn Claude");
            }
            files.push(entry.path());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_switches_are_independent_and_bounded() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/claude-sources-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".claude/skills/demo")).unwrap();
        std::fs::write(root.join("CLAUDE.md"), "INSTRUCTION_MARKER").unwrap();
        std::fs::write(root.join(".claude/skills/demo/SKILL.md"), "SKILL_MARKER").unwrap();
        let mut s = ClaudeConfig::default();
        assert_eq!(project_context(&root, &s).unwrap(), "");
        s.project_skills = true;
        let text = project_context(&root, &s).unwrap();
        assert!(text.contains("SKILL_MARKER") && !text.contains("INSTRUCTION_MARKER"));
        s.project_skills = false;
        s.project_instructions = true;
        let text = project_context(&root, &s).unwrap();
        assert!(!text.contains("SKILL_MARKER") && text.contains("INSTRUCTION_MARKER"));
        std::fs::write(root.join("CLAUDE.md"), "x".repeat(32_001)).unwrap();
        assert!(project_context(&root, &s).is_err());
    }
}
