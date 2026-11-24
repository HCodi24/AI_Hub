use super::super::paths::{check_directories, checked_metadata, valid_skill_name};
use super::{load::find_skill, skill_install_lock};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn shared_skills_dir(config_path: &Path) -> Result<PathBuf> {
    let dir = crate::core::paths::data_dir(config_path)?.join("skills");
    std::path::absolute(dir).context("Không xác định được kho skill chung của MutiS")
}

/// The caller checks the parent directories and validates the skill name before joining it.
pub(super) fn skill_file(directory: &Path) -> Result<Option<PathBuf>> {
    let Some(metadata) = checked_metadata(directory)? else {
        return Ok(None);
    };
    if !metadata.is_dir() {
        bail!("«{}» phải là thư mục skill", directory.display());
    }
    let file = directory.join("SKILL.md");
    match checked_metadata(&file)? {
        Some(metadata) if metadata.is_file() => Ok(Some(file)),
        Some(_) => bail!("«{}» phải là tệp thường", file.display()),
        None => Ok(None),
    }
}

pub(in crate::features::addons) fn shared_skill_names(skills_dir: &Path) -> Vec<String> {
    let Ok(_guard) = skill_install_lock() else {
        return Vec::new();
    };
    if check_directories(skills_dir).is_err() {
        return Vec::new();
    }
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return Vec::new();
    };
    let mut names = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            if !valid_skill_name(&name) {
                return None;
            }
            skill_file(&entry.path()).ok()??;
            Some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

pub(crate) fn shared_skill_catalogue(config_path: &Path) -> String {
    let Ok(skills_dir) = shared_skills_dir(config_path) else {
        return String::new();
    };
    let mut catalogue = String::new();
    for name in shared_skill_names(&skills_dir) {
        let file = skills_dir.join(&name).join("SKILL.md");
        let line = format!("\n- {name}: {}", file.display());
        if catalogue.len() + line.len() > 8000 {
            break;
        }
        catalogue.push_str(&line);
    }
    if catalogue.is_empty() {
        return catalogue;
    }
    format!("Skill trong kho chung của MutiS. Khi cần dùng một skill, hãy đọc SKILL.md và tài nguyên trong cùng thư mục bằng công cụ mà phiên chat cho phép.{catalogue}")
}

pub(crate) fn remove_shared_skill(config_path: &Path, name: &str) -> Result<()> {
    if !valid_skill_name(name) {
        bail!("Tên skill không hợp lệ");
    }
    let _guard = skill_install_lock()?;
    let skills_dir = shared_skills_dir(config_path)?;
    let (_, directory) = find_skill(&skills_dir, name)?
        .with_context(|| format!("Kho chung của MutiS không có skill «{name}»"))?;
    fs::remove_dir_all(directory).with_context(|| format!("Không xóa được skill «{name}»"))
}
