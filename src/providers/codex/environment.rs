use crate::config::{CodexConfig, ProviderConfig};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub fn home(config: &Path, provider: &ProviderConfig) -> Result<PathBuf> {
    let safe = !provider.id.is_empty()
        && provider.id.len() <= 80
        && provider
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b));
    if !safe {
        bail!("Mã nguồn Codex không hợp lệ");
    }
    let path = crate::core::paths::data_dir(config)?
        .join("codex")
        .join(&provider.id);
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

pub fn settings() -> Result<CodexConfig> {
    let path = crate::config::config_file_path()?;
    let config: crate::config::Config = serde_json::from_slice(&std::fs::read(path)?)?;
    Ok(config.codex)
}

pub fn session_prefix(settings: &CodexConfig, provider: &ProviderConfig, folder: &str) -> String {
    let text = format!(
        "{}:{}:{}:{}:{}:{}",
        settings.revision,
        settings.project_skills,
        settings.project_instructions,
        provider.id,
        provider.executable,
        folder
    );
    let hash = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let key = hash.as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    format!("mutis-codex-{key}:")
}

pub fn project_root(folder: &Path) -> PathBuf {
    folder
        .ancestors()
        .find(|p| p.join(".git").exists())
        .unwrap_or(folder)
        .to_path_buf()
}

pub fn launch_args(folder: &Path) -> Vec<OsString> {
    let root = project_root(folder);
    let configs = [
        "cli_auth_credentials_store=\"file\"".to_owned(),
        // Without a Windows sandbox mode Codex falls back to read-only no matter what permission is requested.
        "windows.sandbox=\"elevated\"".into(),
        "project_doc_max_bytes=0".into(),
        "skills.include_instructions=false".into(),
        "skills.bundled.enabled=false".into(),
        "features.apps=false".into(),
        "features.plugins=false".into(),
        "memories.use_memories=false".into(),
        "mcp_servers={}".into(),
        format!(
            "projects.{}.trust_level=\"untrusted\"",
            json!(root.to_string_lossy())
        ),
    ];
    let mut args = vec!["app-server".into(), "--listen".into(), "stdio://".into()];
    for config in configs {
        args.extend([OsString::from("-c"), config.into()]);
    }
    args
}

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

/// Copy the supported file auth cache only. Never copy config, plugins or sessions.
pub fn import_cli_auth(destination: &Path) -> Result<()> {
    let original = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))
        .context("Không xác định được home Codex cá nhân; hãy đăng nhập qua web")?;
    let source = original.join("auth.json");
    if !source.is_file() {
        bail!("Không có đăng nhập CLI dạng file để dùng lại; hãy đăng nhập qua web");
    }
    let raw =
        std::fs::read(&source).context("Không đọc được đăng nhập CLI; hãy đăng nhập qua web")?;
    let value: serde_json::Value =
        serde_json::from_slice(&raw).context("Đăng nhập CLI không hợp lệ")?;
    let auth = filtered_auth(&value)?;
    write_private(&destination.join("auth.json"), &serde_json::to_vec(&auth)?)
}

fn filtered_auth(value: &serde_json::Value) -> Result<serde_json::Value> {
    let tokens = &value["tokens"];
    if !tokens["access_token"]
        .as_str()
        .is_some_and(|s| !s.is_empty())
        || !tokens["refresh_token"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        bail!("CLI chưa có đăng nhập ChatGPT có thể dùng lại; hãy đăng nhập qua web");
    }
    let mut auth = serde_json::Map::new();
    for name in ["auth_mode", "tokens", "last_refresh"] {
        if let Some(v) = value.get(name) {
            auth.insert(name.into(), v.clone());
        }
    }
    Ok(auth.into())
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_import_excludes_configuration() {
        let value = json!({"auth_mode":"chatgpt","tokens":{"access_token":"a","refresh_token":"b"},"mcp_servers":{"personal":{}},"config":"private"});
        let auth = filtered_auth(&value).unwrap();
        assert!(auth.get("mcp_servers").is_none());
        assert!(auth.get("config").is_none());
        assert!(filtered_auth(&json!({"OPENAI_API_KEY":"secret"})).is_err());
    }
    #[test]
    fn permission_changes_invalidate_sessions() {
        let provider = ProviderConfig {
            id: "codex".into(),
            ..Default::default()
        };
        let mut settings = CodexConfig::default();
        let first = session_prefix(&settings, &provider, "C:/repo");
        settings.project_skills = true;
        assert_ne!(first, session_prefix(&settings, &provider, "C:/repo"));
        assert_ne!(
            first,
            session_prefix(&CodexConfig::default(), &provider, "C:/other")
        );
    }

    #[test]
    fn project_sources_are_separate_and_disabled_by_default() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/codex-sources-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".agents/skills/test")).unwrap();
        std::fs::write(root.join("AGENTS.md"), "INSTRUCTION_MARKER").unwrap();
        std::fs::write(root.join(".agents/skills/test/SKILL.md"), "SKILL_MARKER").unwrap();
        let mut settings = CodexConfig::default();
        assert_eq!(project_context(&root, &settings).unwrap(), "");
        settings.project_skills = true;
        let skills = project_context(&root, &settings).unwrap();
        assert!(skills.contains("SKILL_MARKER"));
        assert!(!skills.contains("INSTRUCTION_MARKER"));
        settings.project_skills = false;
        settings.project_instructions = true;
        let instructions = project_context(&root, &settings).unwrap();
        assert!(!instructions.contains("SKILL_MARKER"));
        assert!(instructions.contains("INSTRUCTION_MARKER"));
        let args = launch_args(&root)
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.contains(&"project_doc_max_bytes=0".to_owned()));
        assert!(args.contains(&"skills.include_instructions=false".to_owned()));
    }
}
