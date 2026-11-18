use super::{
    github::{hex, regular_git_file, tree_entries, LinkSkill},
    paths::{check_directories, checked_metadata, safe_component, valid_skill_name},
};
use anyhow::{bail, Context, Result};
use ring::{digest, hmac};
use std::{
    ffi::OsStr,
    fs,
    path::Path,
    sync::{LazyLock, Mutex},
};

/// The catalogue names only the MutiS-managed directories. The provider reads a skill when needed.
pub fn managed_skill_context(skills_dir: &Path) -> String {
    let Ok(skills_dir) = std::path::absolute(skills_dir) else {
        return String::new();
    };
    if check_directories(&skills_dir).is_err() {
        return String::new();
    }
    let Ok(entries) = fs::read_dir(skills_dir) else {
        return String::new();
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    let mut catalogue = String::new();
    for entry in entries {
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|name| valid_skill_name(name)) else {
            continue;
        };
        let path = entry.path();
        if !checked_metadata(&path).is_ok_and(|meta| meta.is_some_and(|meta| meta.is_dir())) {
            continue;
        }
        let file = path.join("SKILL.md");
        if checked_metadata(&file).is_ok_and(|meta| meta.is_some_and(|meta| meta.is_file())) {
            let line = format!("\n- {name}: {}", file.display());
            if catalogue.len() + line.len() > 8000 {
                break;
            }
            catalogue.push_str(&line);
        }
    }
    if catalogue.is_empty() {
        return catalogue;
    }
    format!("\nCác skill do MutiS quản lý. Khi cần dùng một skill, hãy đọc tệp SKILL.md và tìm tệp đi kèm trong cùng thư mục.{catalogue}\n")
}

fn copy_checkout_dir(from: &Path, to: &Path) -> Result<()> {
    let mut directories = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((from, to)) = directories.pop() {
        fs::create_dir(&to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let name = entry.file_name();
            let source = entry.path();
            if !safe_component(&name) {
                bail!("Tên tệp hoặc thư mục không an toàn «{}»", source.display());
            }
            let metadata = checked_metadata(&source)?.context("Tệp trong skill đã biến mất")?;
            if name.eq_ignore_ascii_case(OsStr::new(".git")) {
                continue;
            }
            let destination = to.join(name);
            if metadata.is_dir() {
                directories.push((source, destination));
            } else if metadata.is_file() {
                if checked_metadata(&destination)?.is_some() {
                    bail!(
                        "Hai tệp trong skill có cùng đường dẫn đích «{}»",
                        destination.display()
                    );
                }
                fs::copy(&source, &destination)?;
            } else {
                bail!(
                    "Skill chứa loại tệp không được hỗ trợ «{}»",
                    source.display()
                );
            }
        }
    }
    Ok(())
}

pub enum SkillInstall {
    Installed,
    Conflict(String),
}

static SKILL_INSTALL_LOCK: Mutex<()> = Mutex::new(());
static CONFIRMATION_KEY: LazyLock<hmac::Key> =
    LazyLock::new(|| hmac::Key::new(hmac::HMAC_SHA256, uuid::Uuid::new_v4().as_bytes()));

pub(super) fn skill_install_lock() -> Result<std::sync::MutexGuard<'static, ()>> {
    SKILL_INSTALL_LOCK
        .lock()
        .ok()
        .context("Không mở được khóa cài skill")
}

/// Install only a rediscovered skill. Its directory and destination name are chosen by inspection.
pub fn install_checkout_skill(
    checkout: &Path,
    tree: &[u8],
    skill: &LinkSkill,
    skills_dir: &Path,
    target: &str,
    confirmation: Option<&str>,
) -> Result<SkillInstall> {
    if !valid_skill_name(&skill.name) {
        bail!("Tên thư mục đích của skill không hợp lệ");
    }
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
    let revision = hex(revision.finish().as_ref());
    let from = checkout.join(skill.directory());
    check_directories(&from)?;
    let _guard = skill_install_lock()?;
    let skills_dir = std::path::absolute(skills_dir)?;
    check_directories(&skills_dir)?;
    let destination = skills_dir.join(&skill.name);
    let conflict = || -> Result<Option<String>> {
        Ok(checked_metadata(&destination)?.map(|metadata| {
            let input = format!(
                "{target}\0{}\0{}\0{revision}\0{:?}\0{:?}",
                skill.id,
                destination.display(),
                metadata.created().ok(),
                metadata.modified().ok()
            );
            hex(hmac::sign(&CONFIRMATION_KEY, input.as_bytes()).as_ref())
        }))
    };
    let existing = conflict()?;
    if existing.as_deref() != confirmation {
        if let Some(token) = existing {
            return Ok(SkillInstall::Conflict(token));
        }
        bail!("Đích cài skill đã thay đổi. Hãy thử cài lại.");
    }

    fs::create_dir_all(&skills_dir)?;
    let stage = skills_dir.join(format!(".mutis-skill-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage)?;
    let staged = stage.join("new");
    let previous = stage.join("previous");
    let mut keep_backup = false;
    let result = (|| {
        copy_checkout_dir(&from, &staged)
            .with_context(|| format!("Không chép được skill «{}»", skill.name))?;
        if !checked_metadata(&staged.join("SKILL.md"))?.is_some_and(|metadata| metadata.is_file()) {
            bail!("Bản sao của skill thiếu tệp SKILL.md");
        }
        check_directories(&skills_dir)?;
        if conflict()? != existing {
            bail!("Đích cài skill đã thay đổi. Hãy thử cài lại.");
        }
        if existing.is_some() {
            fs::rename(&destination, &previous).context("Không chuyển được bản skill hiện tại")?;
        }
        if let Err(error) = fs::rename(&staged, &destination) {
            if existing.is_some() {
                if let Err(restore) = fs::rename(&previous, &destination) {
                    keep_backup = true;
                    bail!("Không cài được skill ({error}). Không khôi phục được bản cũ ({restore}). Bản cũ ở «{}»", previous.display());
                }
            }
            return Err(error).context("Không đưa được skill vào thư mục đích");
        }
        Ok(SkillInstall::Installed)
    })();
    if !keep_backup && check_directories(&stage).is_ok() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}
