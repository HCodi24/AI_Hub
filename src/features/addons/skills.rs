use super::{
    github::{hex, regular_git_file, tree_entries, LinkSkill},
    paths::{check_directories, checked_metadata, safe_component, valid_skill_name},
};
use anyhow::{bail, Context, Result};
use ring::{digest, hmac};
use std::{
    ffi::OsStr,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

const MAX_SKILL_BYTES: u64 = 64 * 1024;

pub(crate) fn shared_skills_dir(config_path: &Path) -> Result<PathBuf> {
    let dir = crate::core::paths::data_dir(config_path)?.join("skills");
    std::path::absolute(dir).context("Không xác định được kho skill chung của MutiS")
}

/// The caller checks the parent directories and validates the skill name before joining it.
fn skill_file(directory: &Path) -> Result<Option<PathBuf>> {
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

pub(super) fn shared_skill_names(skills_dir: &Path) -> Vec<String> {
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

fn find_skill(skills_dir: &Path, name: &str) -> Result<Option<(PathBuf, PathBuf)>> {
    let skills_dir = std::path::absolute(skills_dir)?;
    check_directories(&skills_dir)?;
    let directory = skills_dir.join(name);
    let Some(file) = skill_file(&directory)? else {
        return Ok(None);
    };
    let root = skills_dir.canonicalize()?;
    let directory = directory.canonicalize()?;
    let file = file.canonicalize()?;
    if directory.parent() != Some(root.as_path()) || file.parent() != Some(directory.as_path()) {
        bail!("Đường dẫn của skill «{name}» nằm ngoài kho skill");
    }
    Ok(Some((file, directory)))
}

pub(crate) struct LoadedSkill {
    pub directory: PathBuf,
    pub content: String,
}

pub(crate) fn load_skill(
    config_path: &Path,
    providers: &[crate::config::ProviderConfig],
    name: &str,
) -> Result<Option<LoadedSkill>> {
    if !valid_skill_name(name) {
        return Ok(None);
    }
    let _guard = skill_install_lock()?;
    if let Some((file, directory)) = find_skill(&shared_skills_dir(config_path)?, name)? {
        return read_skill(file, directory).map(Some);
    }

    let claude = crate::providers::cli_support::claude_config_dir(config_path)?.join("skills");
    let codex = providers
        .iter()
        .filter(|provider| provider.kind == "codex_cli")
        .filter_map(|provider| {
            crate::providers::codex_cli::legacy_skills_dir(config_path, &provider.id).ok()
        });
    let mut found = None;
    for root in std::iter::once(claude).chain(codex) {
        let Some(skill) = find_skill(&root, name)? else {
            continue;
        };
        if found.as_ref().is_some_and(|previous| previous != &skill) {
            bail!("Có nhiều bản cũ của skill «/{name}». Hãy cài bản bạn muốn dùng vào kho chung trong Cài đặt › Plugin & Skill.");
        }
        found = Some(skill);
    }
    found
        .map(|(file, directory)| read_skill(file, directory))
        .transpose()
}

fn read_skill(file: PathBuf, directory: PathBuf) -> Result<LoadedSkill> {
    let mut bytes = Vec::new();
    fs::File::open(&file)
        .and_then(|file| file.take(MAX_SKILL_BYTES + 1).read_to_end(&mut bytes))
        .with_context(|| format!("Không đọc được «{}»", file.display()))?;
    if bytes.len() as u64 > MAX_SKILL_BYTES {
        bail!("Tệp «{}» vượt giới hạn 64 KiB", file.display());
    }
    let content = String::from_utf8(bytes)
        .with_context(|| format!("Tệp «{}» phải dùng UTF-8", file.display()))?;
    Ok(LoadedSkill {
        directory: crate::core::text::strip_verbatim(directory),
        content,
    })
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
    config_path: &Path,
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
    let skills_dir = shared_skills_dir(config_path)?;
    check_directories(&skills_dir)?;
    let destination = skills_dir.join(&skill.name);
    let conflict = || -> Result<Option<String>> {
        let Some(metadata) = checked_metadata(&destination)? else {
            return Ok(None);
        };
        if !metadata.is_dir() {
            bail!("«{}» phải là thư mục skill", destination.display());
        }
        let input = format!(
            "{}\0{}\0{revision}\0{:?}\0{:?}",
            skill.id,
            destination.display(),
            metadata.created().ok(),
            metadata.modified().ok()
        );
        Ok(Some(hex(
            hmac::sign(&CONFIRMATION_KEY, input.as_bytes()).as_ref()
        )))
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
        skill_file(&staged)?.context("Bản sao của skill thiếu tệp SKILL.md")?;
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
