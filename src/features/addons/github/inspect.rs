use super::super::paths::{check_directories, checked_metadata, read_json, valid_name, valid_skill_name};
use super::{hex, regular_git_file, tree_entries, LinkFound, LinkMarketplace, LinkPlugin, LinkSkill};
use ring::digest;
use std::path::Path;

fn declares_mcp(dir: &Path, tree: &[u8]) -> bool {
    tree_entries(tree).any(|(header, path)| {
        if !regular_git_file(header) {
            return false;
        }
        let name = match path {
            b".mcp.json" => ".mcp.json",
            b"README.md" => "README.md",
            _ => return false,
        };
        let file = dir.join(name);
        if !checked_metadata(&file).is_ok_and(|meta| {
            meta.is_some_and(|meta| meta.is_file() && meta.len() <= 512 * 1024)
        }) {
            return false;
        }
        if name == ".mcp.json" {
            let config = read_json(&file);
            return config
                .get("mcpServers")
                .unwrap_or(&config)
                .as_object()
                .is_some_and(|servers| {
                    servers.values().any(|server| {
                        ["command", "url"].iter().any(|&key| {
                            server[key]
                                .as_str()
                                .is_some_and(|value| !value.trim().is_empty())
                        })
                    })
                });
        }
        // shortcut: README detection requires a server heading and a config example; add other signals as needed.
        std::fs::read_to_string(&file).is_ok_and(|readme| {
            readme.contains("\"mcpServers\"")
                && readme.lines().any(|line| {
                    line.starts_with('#') && line.to_ascii_lowercase().contains("mcp server")
                })
        })
    })
}

fn marketplace_at(file: &Path) -> Option<LinkMarketplace> {
    check_directories(file.parent()?).ok()?;
    if !checked_metadata(file).ok()??.is_file() {
        return None;
    }
    let json = read_json(file);
    let name = json["name"].as_str().filter(|name| valid_name(name))?;
    let plugins = json["plugins"]
        .as_array()?
        .iter()
        .filter_map(|plugin| {
            let name = plugin["name"].as_str().filter(|name| valid_name(name))?;
            let text = |key: &str| plugin[key].as_str().unwrap_or_default();
            let description = if text("description").is_empty() {
                text("category")
            } else {
                text("description")
            };
            Some(LinkPlugin {
                name: name.to_owned(),
                description: description.chars().take(300).collect(),
            })
        })
        .collect::<Vec<_>>();
    (!plugins.is_empty()).then(|| LinkMarketplace {
        name: name.to_owned(),
        plugins,
    })
}

/// Use Git modes as well as filesystem metadata. Git on Windows can check out symlinks as plain files.
pub fn inspect_checkout(dir: &Path, source: &str, tree: &[u8]) -> LinkFound {
    let repo = source
        .rsplit('/')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mut skills = Vec::new();
    for (header, path) in tree_entries(tree) {
        if !regular_git_file(header) {
            continue;
        }
        let Ok(path) = std::str::from_utf8(path) else {
            continue;
        };
        let name = match path {
            "SKILL.md" => Some(repo.as_str()),
            _ => path
                .strip_prefix("skills/")
                .and_then(|path| path.strip_suffix("/SKILL.md")),
        };
        if let Some(name) = name.filter(|name| valid_skill_name(name)) {
            let file = dir.join(path);
            if check_directories(file.parent().expect("skill parent")).is_ok()
                && checked_metadata(&file).is_ok_and(|meta| meta.is_some_and(|meta| meta.is_file()))
            {
                let key = format!("{}\0{path}", source.to_ascii_lowercase());
                skills.push(LinkSkill {
                    id: hex(digest::digest(&digest::SHA256, key.as_bytes()).as_ref()),
                    name: name.to_owned(),
                    path: path.to_owned(),
                });
            }
        }
    }
    skills.sort_by(|first, second| first.path.cmp(&second.path));
    LinkFound {
        claude: marketplace_at(&dir.join(".claude-plugin/marketplace.json")),
        codex: marketplace_at(&dir.join(".agents/plugins/marketplace.json")),
        skills,
        mcp: declares_mcp(dir, tree),
    }
}
