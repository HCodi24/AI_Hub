use serde::Serialize;
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tokio::time::timeout;

use crate::providers::cli_support;

#[derive(Clone, Copy)]
pub struct Action {
    pub id: &'static str,
    pub label: &'static str,
    pub command: &'static str,
    pub args: &'static [&'static str],
    pub modifies: &'static str,
    pub needs_folder: bool,
    pub backup: Option<&'static str>,
}

pub struct Definition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub homepage: &'static str,
    pub executable: &'static str,
    pub version_args: &'static [&'static str],
    pub actions: &'static [Action],
}

const ACTIONS: &[Action] = &[
    Action {
        id: "install",
        label: "Cài đặt",
        command: "npm",
        args: &["i", "-g", "@colbymchenry/codegraph"],
        modifies: "npm global packages",
        needs_folder: false,
        backup: None,
    },
    Action {
        id: "connect_codex",
        label: "Kết nối Codex",
        command: "codegraph",
        args: &["install", "--target=codex", "--yes"],
        modifies: "~/.codex/config.toml",
        needs_folder: false,
        backup: Some("codex/config.toml"),
    },
    Action {
        id: "connect_claude",
        label: "Kết nối Claude",
        command: "codegraph",
        args: &["install", "--target=claude", "--yes"],
        modifies: "~/.claude.json",
        needs_folder: false,
        backup: Some(".claude.json"),
    },
    Action {
        id: "index_folder",
        label: "Lập chỉ mục thư mục",
        command: "codegraph",
        args: &["init"],
        modifies: "<thư mục>/.codegraph/",
        needs_folder: true,
        backup: None,
    },
    Action {
        id: "upgrade",
        label: "Nâng cấp",
        command: "codegraph",
        args: &["upgrade"],
        modifies: "CodeGraph installation",
        needs_folder: false,
        backup: None,
    },
    Action {
        id: "uninstall",
        label: "Gỡ cài đặt",
        command: "npm",
        args: &["rm", "-g", "@colbymchenry/codegraph"],
        modifies: "npm global packages",
        needs_folder: false,
        backup: None,
    },
];

pub const CATALOG: &[Definition] = &[Definition {
    id: "codegraph",
    name: "CodeGraph",
    description: "Lập chỉ mục mã nguồn để hỗ trợ công cụ AI hiểu dự án.",
    homepage: "https://github.com/colbymchenry/codegraph",
    executable: "codegraph",
    version_args: &["--version"],
    actions: ACTIONS,
}];

#[derive(Serialize)]
pub struct Card {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub homepage: &'static str,
    pub installed: bool,
    pub version: Option<String>,
    pub codex: bool,
    pub claude: bool,
    pub folder_indexed: Option<bool>,
    pub actions: Vec<ActionView>,
}

#[derive(Serialize)]
pub struct ActionView {
    pub id: &'static str,
    pub label: &'static str,
    pub command: String,
    pub modifies: &'static str,
}

pub async fn status(folder: Option<&str>) -> Vec<Card> {
    let mut cards = Vec::with_capacity(CATALOG.len());
    for definition in CATALOG {
        let version =
            if let Some(path) = cli_support::resolve_executable(definition.executable, None) {
                run_version(&path, definition.version_args).await
            } else {
                None
            };
        let home = dirs::home_dir().unwrap_or_default();
        let codex = std::fs::read_to_string(home.join(".codex/config.toml"))
            .map(|contents| contents.contains("[mcp_servers.codegraph"))
            .unwrap_or(false);
        let claude = std::fs::read_to_string(home.join(".claude.json"))
            .map(|contents| {
                contents.contains("\"mcpServers\"") && contents.contains("\"codegraph\"")
            })
            .unwrap_or(false);

        cards.push(Card {
            id: definition.id,
            name: definition.name,
            description: definition.description,
            homepage: definition.homepage,
            installed: version.is_some(),
            version,
            codex,
            claude,
            folder_indexed: folder.map(|folder| {
                PathBuf::from(folder)
                    .join(".codegraph/codegraph.db")
                    .is_file()
            }),
            actions: definition
                .actions
                .iter()
                .map(|action| ActionView {
                    id: action.id,
                    label: action.label,
                    command: format!("{} {}", action.command, action.args.join(" ")),
                    modifies: action.modifies,
                })
                .collect(),
        });
    }
    cards
}

async fn run_version(path: &std::path::Path, args: &[&str]) -> Option<String> {
    let arguments = args.iter().map(OsString::from).collect::<Vec<_>>();
    let mut command = cli_support::command_for(path, &arguments);
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(10), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!version.is_empty()).then_some(version)
}

pub fn action(id: &str, action_id: &str) -> Option<(&'static Definition, &'static Action)> {
    let definition = CATALOG.iter().find(|definition| definition.id == id)?;
    Some((
        definition,
        definition
            .actions
            .iter()
            .find(|action| action.id == action_id)?,
    ))
}

pub fn resolve(name: &str) -> Option<PathBuf> {
    let names = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_owned(),
        ]
    } else {
        vec![name.to_owned()]
    };
    let mut directories =
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).collect::<Vec<_>>();
    #[cfg(windows)]
    {
        for key in ["APPDATA", "USERPROFILE", "LOCALAPPDATA"] {
            if let Some(path) = std::env::var_os(key).map(PathBuf::from) {
                directories.push(match key {
                    "APPDATA" => path.join("npm"),
                    "USERPROFILE" => path.join(".codegraph").join("bin"),
                    _ => path.join("codegraph").join("bin"),
                });
            }
        }
    }
    names
        .iter()
        .flat_map(|name| {
            directories
                .iter()
                .map(move |directory| directory.join(name))
        })
        .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_actions_are_fixed_and_resolvable() {
        let (definition, install) = action("codegraph", "install").expect("catalog install action");
        assert_eq!(definition.executable, "codegraph");
        assert_eq!(install.args, ["i", "-g", "@colbymchenry/codegraph"]);
        assert_eq!(install.label, "Cài đặt");
        assert!(action("codegraph", "arbitrary-command").is_none());
    }

    #[test]
    fn catalog_strings_do_not_contain_mojibake() {
        for definition in CATALOG {
            assert!(!definition.name.contains('\u{00c3}'));
            assert!(!definition.description.contains('\u{00c3}'));
            for action in definition.actions {
                assert!(!action.label.contains('\u{00c3}'));
            }
        }
    }
}
