use super::catalog::CATALOG;
use crate::providers::cli_support;
use serde::Serialize;
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tokio::time::timeout;

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
