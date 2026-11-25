use super::super::paths::{check_directories, valid_skill_name};
use super::{builtin::builtin_skill, shared::skill_file, shared_skills_dir, skill_install_lock};
use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

const MAX_SKILL_BYTES: u64 = 64 * 1024;

pub(super) fn find_skill(skills_dir: &Path, name: &str) -> Result<Option<(PathBuf, PathBuf)>> {
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
    pub directory: Option<PathBuf>,
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
    if let Some(content) = builtin_skill(name) {
        return Ok(Some(LoadedSkill {
            directory: None,
            content: content.to_owned(),
        }));
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
        directory: Some(crate::core::text::strip_verbatim(directory)),
        content,
    })
}
