use super::{
    copy_checkout_dir, shared::skill_file, shared_skills_dir, skill_install_lock, swap_into_place,
    SkillInstall, CONFIRMATION_KEY,
};
use super::super::{
    github::{hex, regular_git_file, tree_entries, LinkSkill},
    paths::{check_directories, checked_metadata, safe_component, valid_skill_name},
};
use anyhow::{bail, Context, Result};
use ring::{digest, hmac};
use std::{ffi::OsStr, path::Path};

/// Install only a rediscovered skill. Its directory and destination name are chosen by inspection.
pub fn install_checkout_skill(
    checkout: &Path,
    tree: &[u8],
    skill: &LinkSkill,
    config_path: &Path,
    confirmation: Option<&str>,
) -> Result<SkillInstall> {
    if !valid_skill_name(&skill.name) {
        bail!("Tên thư mục đích của skill không hợp lệ");
    }
    let revision = skill_revision(tree, skill)?;
    let from = checkout.join(skill.directory());
    check_directories(&from)?;
    let _guard = skill_install_lock()?;
    let skills_dir = shared_skills_dir(config_path)?;
    check_directories(&skills_dir)?;
    let destination = skills_dir.join(&skill.name);
    let existing = existing_confirmation(&destination, &skill.id, &revision)?;
    if existing.as_deref() != confirmation {
        if let Some(token) = existing {
            return Ok(SkillInstall::Conflict(token));
        }
        bail!("Đích cài skill đã thay đổi. Hãy thử cài lại.");
    }

    stage_and_swap(&from, &skills_dir, &destination, skill, &revision, existing)
}

fn skill_revision(tree: &[u8], skill: &LinkSkill) -> Result<String> {
    let prefix = skill
        .path
        .strip_suffix("SKILL.md")
        .context("Đường dẫn skill không hợp lệ")?;
    let mut revision = digest::Context::new(&digest::SHA256);
    for (header, path) in tree_entries(tree) {
        let Some(relative) = path.strip_prefix(prefix.as_bytes()) else {
            continue;
        };
        let relative =
            std::str::from_utf8(relative).context("Tên tệp trong skill phải dùng UTF-8")?;
        if !relative
            .split('/')
            .all(|name| safe_component(OsStr::new(name)))
        {
            bail!("Đường dẫn trong skill không an toàn «{relative}»");
        }
        if !regular_git_file(header) {
            bail!("Skill chứa liên kết tượng trưng hoặc mô-đun Git con «{relative}»");
        }
        revision.update(header);
        revision.update(b"\t");
        revision.update(path);
        revision.update(b"\0");
    }
    Ok(hex(revision.finish().as_ref()))
}

fn existing_confirmation(
    destination: &Path,
    skill_id: &str,
    revision: &str,
) -> Result<Option<String>> {
    let Some(metadata) = checked_metadata(destination)? else {
        return Ok(None);
    };
    if !metadata.is_dir() {
        bail!("«{}» phải là thư mục skill", destination.display());
    }
    let input = format!(
        "{}\0{}\0{revision}\0{:?}\0{:?}",
        skill_id,
        destination.display(),
        metadata.created().ok(),
        metadata.modified().ok()
    );
    Ok(Some(hex(
        hmac::sign(&CONFIRMATION_KEY, input.as_bytes()).as_ref()
    )))
}

fn stage_and_swap(
    from: &Path,
    skills_dir: &Path,
    destination: &Path,
    skill: &LinkSkill,
    revision: &str,
    existing: Option<String>,
) -> Result<SkillInstall> {
    swap_into_place(
        skills_dir,
        destination,
        |staged| {
            copy_checkout_dir(from, staged)
                .with_context(|| format!("Không chép được skill «{}»", skill.name))?;
            skill_file(staged)?.context("Bản sao của skill thiếu tệp SKILL.md")?;
            Ok(())
        },
        || {
            if existing_confirmation(destination, &skill.id, revision)? != existing {
                bail!("Đích cài skill đã thay đổi. Hãy thử cài lại.");
            }
            Ok(existing.is_some())
        },
    )
    .map(|()| SkillInstall::Installed)
}
