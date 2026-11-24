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

/// Codex plugins are off unless the user installed some into AI Hub's own Codex home (Settings › Add-ons).
pub fn has_plugins(home: &Path) -> bool {
    std::fs::read_dir(home.join("plugins/cache")).is_ok_and(|mut entries| entries.next().is_some())
}

pub fn launch_args(folder: &Path, plugins: bool) -> Vec<OsString> {
    let root = project_root(folder);
    let configs = [
        "cli_auth_credentials_store=\"file\"".to_owned(),
        // Without a Windows sandbox mode Codex falls back to read-only no matter what permission is requested.
        "windows.sandbox=\"elevated\"".into(),
        "project_doc_max_bytes=0".into(),
        // With plugins installed, let Codex list their skills itself (as Orca does); without any it stays off so repo skills are never auto-loaded.
        format!("skills.include_instructions={plugins}"),
        "skills.bundled.enabled=false".into(),
        "features.apps=false".into(),
        format!("features.plugins={plugins}"),
        "memories.use_memories=false".into(),
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

/// `[mcp_servers.NAME]` table header -> NAME (plain or quoted).
fn mcp_header(line: &str) -> Option<&str> {
    let name = line.trim().strip_prefix("[mcp_servers.")?.strip_suffix(']')?;
    let name = name.trim_matches('"');
    (!name.contains('.') && !name.is_empty()).then_some(name)
}

/// MCP servers in the Codex home's `config.toml` as `{name, summary, enabled}` (Codex itself loads them; `enabled = false` turns one off).
pub fn mcp_servers(home: &Path) -> Vec<serde_json::Value> {
    let text = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let (mut out, mut current) = (Vec::<serde_json::Value>::new(), None::<usize>);
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            current = mcp_header(line).map(|name| { out.push(json!({"name": name, "summary": "", "enabled": true})); out.len() - 1 });
        } else if let Some(i) = current {
            if let Some(value) = line.strip_prefix("command") .and_then(|v| v.trim_start().strip_prefix('=')) { out[i]["summary"] = json!(value.trim().trim_matches('"')); }
            if line.replace(' ', "") == "enabled=false" { out[i]["enabled"] = json!(false); }
        }
    }
    out
}

/// Writes `enabled = true|false` into one `[mcp_servers.NAME]` table of the Codex home's `config.toml`.
pub fn set_mcp_enabled(home: &Path, name: &str, on: bool) -> Result<()> {
    let path = home.join("config.toml");
    let text = std::fs::read_to_string(&path).context("Chưa có config.toml của Codex")?;
    let mut lines = text.lines().map(str::to_owned).collect::<Vec<_>>();
    let start = lines.iter().position(|l| mcp_header(l) == Some(name)).with_context(|| format!("Codex chưa có MCP «{name}»"))?;
    let end = lines[start + 1..].iter().position(|l| l.trim().starts_with('[')).map_or(lines.len(), |i| start + 1 + i);
    let line = format!("enabled = {on}");
    match (start + 1..end).find(|&i| lines[i].replace(' ', "").starts_with("enabled=")) {
        Some(i) => lines[i] = line,
        None => lines.insert(start + 1, line),
    }
    std::fs::write(&path, lines.join("
") + "
")?;
    Ok(())
}

/// Plugins that sit in the Codex home cache, as `{pluginId, version, enabled}`: the fallback for when `codex plugin list` shows none.
pub fn cached_plugins(home: &Path) -> Vec<serde_json::Value> {
    let config = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let mut out = Vec::new();
    let dirs = |p: PathBuf| std::fs::read_dir(p).into_iter().flatten().flatten().filter(|e| e.path().is_dir()).collect::<Vec<_>>();
    for market in dirs(home.join("plugins/cache")) {
        for plugin in dirs(market.path()) {
            let id = format!("{}@{}", plugin.file_name().to_string_lossy(), market.file_name().to_string_lossy());
            let mut versions = dirs(plugin.path()).into_iter().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>();
            versions.sort();
            // Off only when config.toml says `enabled = false` right under this plugin's table.
            let header = format!("[plugins.\"{id}\"]");
            let off = config.split(&header).nth(1).and_then(|rest| rest.lines().skip(1).take_while(|l| !l.trim().starts_with('[')).find(|l| l.contains("enabled"))).is_some_and(|l| l.contains("false"));
            out.push(json!({"pluginId": id, "version": versions.pop().unwrap_or_default(), "enabled": !off}));
        }
    }
    out
}

/// Tell the model which enabled plugin skills exist (name, one-line purpose, path) so it reads one on demand.
/// Codex itself is launched with skill instructions off, so without this it never learns the plugins are there.
pub fn plugin_index(home: &Path) -> String {
    let Ok(config) = std::fs::read_to_string(home.join("config.toml")) else { return String::new() };
    let (mut enabled, mut current) = (Vec::new(), None::<String>);
    for line in config.lines().map(str::trim) {
        if let Some(id) = line.strip_prefix("[plugins.\"").and_then(|l| l.strip_suffix("\"]")) {
            current = Some(id.to_owned());
        } else if line.starts_with('[') {
            current = None;
        } else if line.replace(' ', "") == "enabled=true" {
            enabled.extend(current.take());
        }
    }
    let mut plugins = Vec::new();
    for id in enabled {
        let Some((name, market)) = id.split_once('@') else { continue };
        let Ok(versions) = std::fs::read_dir(home.join("plugins/cache").join(market).join(name)) else { continue };
        let mut versions = versions.flatten().map(|e| e.path()).collect::<Vec<_>>();
        versions.sort();
        plugins.extend(versions.pop().map(|v| (name.to_owned(), v)));
    }
    skill_index(plugins)
}

/// The index text for plugins given as (name, install folder); shared by Codex and Claude.
pub fn skill_index(plugins: Vec<(String, PathBuf)>) -> String {
    let (mut out, mut seen) = (String::new(), std::collections::HashSet::new());
    for (name, version) in plugins {
        let mut files = Vec::new();
        find_skill_files(&version, &mut files, 0);
        for file in files {
            let Ok(text) = std::fs::read_to_string(&file) else { continue };
            let field = |key: &str| text.lines().take(12).find_map(|l| l.strip_prefix(key)).map(|v| v.trim().trim_matches('"').to_owned());
            let skill = field("name:").unwrap_or_default();
            if skill.is_empty() || !seen.insert(format!("{name}:{skill}")) || out.len() > 8000 { continue }
            let about = field("description:").unwrap_or_default().chars().take(160).collect::<String>();
            out.push_str(&format!("- {name}:{skill} — {about} ({})
", file.display()));
        }
    }
    if out.is_empty() { return out }
    format!("
Installed plugin skills (enabled in Settings). When a request matches one, read its SKILL.md and follow it:
{out}")
}

fn find_skill_files(dir: &Path, files: &mut Vec<PathBuf>, depth: usize) {
    if depth > 5 || files.len() >= 40 { return }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() { continue }
        let name = entry.file_name();
        if kind.is_dir() && name != ".git" && name != "node_modules" {
            find_skill_files(&entry.path(), files, depth + 1);
        } else if name == "SKILL.md" {
            files.push(entry.path());
        }
    }
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
    crate::core::fs::write_atomic(
        &destination.join("auth.json"),
        &serde_json::to_vec(&auth)?,
        true,
    )
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
    fn mcp_servers_are_listed_and_toggled_in_config_toml() {
        let home = std::env::temp_dir().join(format!("aihub-codex-mcp-{}", std::process::id()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("config.toml"), "[mcp_servers.a]
command = \"x\"
[mcp_servers.a.env]
K = \"v\"
[mcp_servers.b]
command = \"y\"
enabled = false
").unwrap();
        assert_eq!(mcp_servers(&home).iter().map(|s| s["enabled"] == true).collect::<Vec<_>>(), [true, false]);
        set_mcp_enabled(&home, "b", true).unwrap();
        set_mcp_enabled(&home, "a", false).unwrap();
        let after = mcp_servers(&home);
        std::fs::remove_dir_all(&home).ok();
        assert_eq!(after.iter().map(|s| s["enabled"] == true).collect::<Vec<_>>(), [false, true]);
        assert_eq!(after[0]["summary"], "x");
    }
    #[test]
    fn plugin_index_lists_enabled_plugin_skills_only() {
        let home = std::env::temp_dir().join(format!("aihub-plugin-index-{}", std::process::id()));
        for (plugin, skill) in [("on", "alpha"), ("off", "beta")] {
            let dir = home.join(format!("plugins/cache/m/{plugin}/1.0/skills/{skill}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), format!("---
name: {skill}
description: \"does {skill}\"
---
")).unwrap();
        }
        std::fs::write(home.join("config.toml"), "[plugins.\"on@m\"]
enabled = true
[plugins.\"off@m\"]
enabled = false
").unwrap();
        let index = plugin_index(&home);
        std::fs::remove_dir_all(&home).ok();
        assert!(index.contains("on:alpha") && index.contains("does alpha") && !index.contains("beta"));
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
        let args = launch_args(&root, false)
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.contains(&"features.plugins=false".to_owned()));
        assert!(launch_args(&root, true)
            .iter()
            .any(|arg| arg == "features.plugins=true"));
        assert!(launch_args(&root, true)
            .iter()
            .any(|arg| arg == "skills.include_instructions=true"));
        assert!(args.contains(&"project_doc_max_bytes=0".to_owned()));
        assert!(args.contains(&"skills.include_instructions=false".to_owned()));
    }
}
