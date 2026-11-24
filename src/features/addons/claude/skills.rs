use super::super::{
    paths::{check_directories, checked_metadata, valid_skill_name},
    skills::{copy_checkout_dir, skill_install_lock, swap_into_place},
};
use anyhow::{bail, Context, Result};
use std::path::Path;

fn has_skill_file(directory: &Path) -> Result<bool> {
    Ok(checked_metadata(&directory.join("SKILL.md"))?.is_some_and(|metadata| metadata.is_file()))
}

/// Copy one skill folder from the user's Claude Code into AI Hub's Claude config (replacing an older copy).
pub fn copy_skill(personal_dir: &Path, hub_dir: &Path, name: &str) -> Result<()> {
    if !valid_skill_name(name) {
        bail!("Tên skill không hợp lệ");
    }
    let _guard = skill_install_lock()?;
    let from = personal_dir.join("skills").join(name);
    check_directories(&from)?;
    if !has_skill_file(&from)? {
        bail!("Không tìm thấy skill «{name}» trong Claude Code của bạn");
    }
    let skills_dir = hub_dir.join("skills");
    check_directories(&skills_dir)?;
    let destination = skills_dir.join(name);
    swap_into_place(
        &skills_dir,
        &destination,
        |staged| {
            copy_checkout_dir(&from, staged)
                .with_context(|| format!("Không chép được skill «{name}»"))?;
            if !has_skill_file(staged)? {
                bail!("Bản sao của skill thiếu tệp SKILL.md");
            }
            Ok(())
        },
        || match checked_metadata(&destination)? {
            Some(metadata) if metadata.is_dir() => Ok(true),
            Some(_) => bail!("«{}» phải là thư mục skill", destination.display()),
            None => Ok(false),
        },
    )
}

pub fn remove_skill(hub_dir: &Path, name: &str) -> Result<()> {
    if !valid_skill_name(name) {
        bail!("Tên skill không hợp lệ");
    }
    let _guard = skill_install_lock()?;
    let dir = hub_dir.join("skills").join(name);
    if !dir.join("SKILL.md").is_file() {
        bail!("MutiS không có skill «{name}»");
    }
    std::fs::remove_dir_all(dir).with_context(|| format!("Không xoá được skill «{name}»"))
}
