mod files;
mod shell;

use super::{emit_text, mcp::McpTools, ProviderEvent};
use crate::chat::Permission;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct AgentTools {
    root: PathBuf,
    permission: Permission,
    max_file_lines: u32,
    files: bool,
    mcp: McpTools,
    snapshots: BTreeMap<String, Option<String>>,
    scope: Vec<String>,
}

fn normalize(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("./").to_lowercase()
}

/// With a scope, edits are limited to the listed files; `write_file` may also create a NEW file in a listed file's folder.
fn in_scope(root: &std::path::Path, scope: &[String], path: &str, creating: bool) -> bool {
    let path = normalize(path);
    let folder = |p: &str| p.rsplit_once('/').map_or(String::new(), |(dir, _)| dir.to_owned());
    scope.iter().map(|entry| normalize(entry)).any(|entry| {
        entry == path || (creating && !root.join(&path).exists() && folder(&entry) == folder(&path))
    })
}

impl AgentTools {
    pub fn new(
        root: PathBuf,
        permission: Permission,
        max_file_lines: u32,
        files: bool,
        mcp: McpTools,
    ) -> Self {
        Self {
            root,
            permission,
            max_file_lines,
            files,
            mcp,
            snapshots: BTreeMap::new(),
            scope: Vec::new(),
        }
    }

    /// Limit file edits to these paths (empty = no limit).
    pub fn with_scope(mut self, scope: Vec<String>) -> Self {
        self.scope = scope;
        self
    }

    pub fn openai_definitions(&self) -> Vec<Value> {
        let mut tools = if self.files {
            files::openai_definitions(self.permission != Permission::Read)
        } else {
            Vec::new()
        };
        if self.files && self.permission == Permission::Full {
            tools.push(shell::openai_definition());
        }
        tools.extend(self.mcp.openai_definitions());
        tools
    }

    pub fn anthropic_definitions(&self) -> Vec<Value> {
        let mut tools = if self.files {
            files::anthropic_definitions(self.permission != Permission::Read)
        } else {
            Vec::new()
        };
        if self.files && self.permission == Permission::Full {
            tools.push(shell::anthropic_definition());
        }
        tools.extend(self.mcp.anthropic_definitions());
        tools
    }

    pub async fn execute(
        &mut self,
        name: &str,
        args: Value,
        events: &mpsc::Sender<ProviderEvent>,
        cancel: &CancellationToken,
    ) -> String {
        if let Some((server, tool)) = self
            .mcp
            .find(name)
            .map(|tool| (tool.server.clone(), tool.tool.clone()))
        {
            let _ = emit_text(events, "log", format!("MCP {server} › {tool}")).await;
            return tokio::select! {
                _ = cancel.cancelled() => "error: Đã dừng".into(),
                result = self.mcp.call(name, args) => result.unwrap_or_else(|error| format!("error: {error:#}")),
            };
        }
        if name == "run_command" {
            if self.permission != Permission::Full {
                return "error: lệnh chỉ chạy được ở quyền Toàn quyền".into();
            }
            if !self.files {
                return format!("error: Công cụ không tồn tại: {name}");
            }
            return shell::run_command(&self.root, &args, events, cancel).await;
        }
        if !self.files {
            return format!("error: Công cụ không tồn tại: {name}");
        }

        let path = args["path"].as_str().unwrap_or(".");
        if name == "read_file" {
            if let Ok(file) = files::safe_path(&self.root, path) {
                if let Ok(text) = fs::read_to_string(file) {
                    let total = text.lines().count();
                    let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
                    if args["start_line"].is_null()
                        && args["end_line"].is_null()
                        && total > self.max_file_lines as usize
                    {
                        let _ =
                            emit_text(events, "log", format!("Cảnh báo: {path} dài {total} dòng"))
                                .await;
                    }
                    let end = args["end_line"]
                        .as_u64()
                        .map(|value| value as usize)
                        .unwrap_or(total)
                        .min(total)
                        .min(start.saturating_add(399));
                    let _ = emit_text(events, "log", format!("Đọc {path} {start}–{end}")).await;
                }
            }
        }
        if matches!(name, "edit_file" | "write_file") && self.permission == Permission::Read {
            let _ = emit_text(events, "log", format!("Từ chối: {path} (chế độ chỉ đọc)")).await;
            return "error: chế độ chỉ đọc — không được sửa hoặc ghi file".into();
        }
        if matches!(name, "edit_file" | "write_file") && !self.scope.is_empty() && !in_scope(&self.root, &self.scope, path, name == "write_file") {
            let _ = emit_text(events, "log", format!("Từ chối: {path} (ngoài phạm vi được giao)")).await;
            return format!("error: {path} is outside the files you were given ({}). Edit only those files; write_file may also create a new file next to them.", self.scope.join(", "));
        }
        if matches!(name, "edit_file" | "write_file") {
            match files::safe_path(&self.root, path) {
                Ok(_) => {
                    let _ = emit_text(events, "log", format!("Sửa {path}")).await;
                }
                Err(error) => {
                    let _ = emit_text(events, "log", format!("Từ chối: {path} ({error})")).await;
                }
            }
        }
        files::run_tool(
            &self.root,
            name,
            &args,
            self.max_file_lines,
            &mut self.snapshots,
        )
    }

    pub fn diff(&self) -> String {
        files::make_diff(&self.root, &self.snapshots)
    }

    pub fn change_summaries(&self) -> Vec<(String, usize, usize)> {
        self.snapshots
            .iter()
            .map(|(path, before)| {
                let after = fs::read_to_string(self.root.join(path)).unwrap_or_default();
                let (added, removed) = files::line_counts(before.as_deref().unwrap_or(""), &after);
                (path.clone(), added, removed)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn scope_limits_edits_to_listed_files_and_new_files_beside_them() {
        let root = std::env::temp_dir().join(format!("ai-hub-scope-{}", std::process::id()));
        std::fs::create_dir_all(root.join("web/src")).unwrap();
        std::fs::write(root.join("web/src/other.ts"), "x").unwrap();
        let scope = vec!["web/src/a.tsx".to_owned()];
        assert!(in_scope(&root, &scope, "./web/src/A.tsx", false));
        assert!(!in_scope(&root, &scope, "web/src/other.ts", false), "unlisted existing file");
        assert!(!in_scope(&root, &scope, "web/src/new.tsx", false), "edit_file never creates");
        assert!(in_scope(&root, &scope, "web/src/new.tsx", true), "new file beside a listed one");
        assert!(!in_scope(&root, &scope, "web/src/other.ts", true), "write_file over an unlisted existing file");
        assert!(!in_scope(&root, &scope, "src/new.rs", true), "other folder");
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn definitions_follow_file_and_command_permissions_for_both_apis() {
        let tools = |permission, files| {
            let mcp = tokio::runtime::Runtime::new().unwrap().block_on(async {
                let (events, _receiver) = mpsc::channel(1);
                McpTools::start(&[], ".", permission != Permission::Read, &events).await
            });
            AgentTools::new(PathBuf::from("."), permission, 400, files, mcp)
        };
        let read = tools(Permission::Read, true);
        let write = tools(Permission::Write, true);
        let full = tools(Permission::Full, true);
        let mcp_only = tools(Permission::Full, false);
        let openai_names = |definitions: Vec<Value>| {
            definitions
                .iter()
                .map(|tool| tool["function"]["name"].as_str().unwrap().to_owned())
                .collect::<Vec<String>>()
        };
        assert_eq!(
            openai_names(read.openai_definitions()),
            vec!["list_dir", "read_file"]
        );
        assert_eq!(
            openai_names(write.openai_definitions()),
            vec!["list_dir", "read_file", "edit_file", "write_file"]
        );
        assert_eq!(
            openai_names(full.openai_definitions()),
            vec![
                "list_dir",
                "read_file",
                "edit_file",
                "write_file",
                "run_command"
            ]
        );
        assert!(mcp_only.openai_definitions().is_empty());
        assert_eq!(
            full.anthropic_definitions()
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned())
                .collect::<Vec<_>>(),
            vec![
                "list_dir",
                "read_file",
                "edit_file",
                "write_file",
                "run_command"
            ]
        );
        assert!(mcp_only.anthropic_definitions().is_empty());
    }

    #[tokio::test]
    async fn run_command_is_rejected_below_full_and_in_mcp_only_mode() {
        let (events, _receiver) = mpsc::channel(1);
        let cancel = CancellationToken::new();
        for (permission, files, expected) in [
            (
                Permission::Read,
                true,
                "error: lệnh chỉ chạy được ở quyền Toàn quyền",
            ),
            (
                Permission::Write,
                true,
                "error: lệnh chỉ chạy được ở quyền Toàn quyền",
            ),
            (
                Permission::Full,
                false,
                "error: Công cụ không tồn tại: run_command",
            ),
        ] {
            let mcp = super::super::mcp::McpTools::start(&[], ".", true, &events).await;
            let mut tools = AgentTools::new(PathBuf::from("."), permission, 400, files, mcp);
            assert_eq!(
                tools
                    .execute(
                        "run_command",
                        json!({"command":"echo no"}),
                        &events,
                        &cancel
                    )
                    .await,
                expected
            );
        }
    }

    #[test]
    fn anthropic_file_tools_use_input_schema() {
        let mcp = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(async { McpTools::start(&[], ".", true, &mpsc::channel(1).0).await });
        let tools = AgentTools::new(PathBuf::from("."), Permission::Read, 400, true, mcp);
        let definitions = tools.anthropic_definitions();
        assert!(definitions
            .iter()
            .all(|tool| tool["input_schema"].is_object()));
        assert!(definitions
            .iter()
            .all(|tool| tool.get("parameters").is_none()));
    }
}
