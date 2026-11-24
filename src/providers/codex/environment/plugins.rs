use serde_json::json;
use std::path::{Path, PathBuf};

/// Plugins that sit in the Codex home cache, as `{pluginId, version, enabled}`: the fallback for when `codex plugin list` shows none.
pub fn cached_plugins(home: &Path) -> Vec<serde_json::Value> {
    let config = std::fs::read_to_string(home.join("config.toml")).unwrap_or_default();
    let mut out = Vec::new();
    let dirs = |p: PathBuf| {
        std::fs::read_dir(p)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect::<Vec<_>>()
    };
    for market in dirs(home.join("plugins/cache")) {
        for plugin in dirs(market.path()) {
            let id = format!(
                "{}@{}",
                plugin.file_name().to_string_lossy(),
                market.file_name().to_string_lossy()
            );
            let mut versions = dirs(plugin.path())
                .into_iter()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect::<Vec<_>>();
            versions.sort();
            // Off only when config.toml says `enabled = false` right under this plugin's table.
            let header = format!("[plugins.\"{id}\"]");
            let off = config
                .split(&header)
                .nth(1)
                .and_then(|rest| {
                    rest.lines()
                        .skip(1)
                        .take_while(|l| !l.trim().starts_with('['))
                        .find(|l| l.contains("enabled"))
                })
                .is_some_and(|l| l.contains("false"));
            out.push(json!({
                "pluginId": id,
                "version": versions.pop().unwrap_or_default(),
                "enabled": !off
            }));
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
            out.push_str(&format!("- {name}:{skill} — {about} ({})\n", file.display()));
        }
    }
    if out.is_empty() { return out }
    format!("\nInstalled plugin skills (enabled in Settings). When a request matches one, read its SKILL.md and follow it:\n{out}")
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
