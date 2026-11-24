mod files;
mod shell;

use super::{emit_text, mcp::McpTools, ProviderEvent};
use crate::chat::Permission;
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod scope;

use self::scope::in_scope;

pub struct AgentTools {
    root: PathBuf,
    permission: Permission,
    max_file_lines: u32,
    files: bool,
    mcp: McpTools,
    snapshots: BTreeMap<String, Option<String>>,
    scope: Vec<String>,
}

pub(super) fn openai_tool(tool: Value) -> Value {
    serde_json::json!({
        "type":"function",
        "function":{
            "name":tool["name"],
            "description":tool["description"],
            "parameters":tool["input_schema"]
        }
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
            return self
                .execute_mcp(name, args, events, cancel, (server, tool))
                .await;
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
        self.execute_file_tool(name, &args, events).await
    }

    async fn execute_mcp(
        &mut self,
        name: &str,
        args: Value,
        events: &mpsc::Sender<ProviderEvent>,
        cancel: &CancellationToken,
        target: (String, String),
    ) -> String {
        let (server, tool) = target;
        let _ = emit_text(events, "log", format!("MCP {server} › {tool}")).await;
        tokio::select! {
            _ = cancel.cancelled() => "error: Đã dừng".into(),
            result = self.mcp.call(name, args) => result.unwrap_or_else(|error| format!("error: {error:#}")),
        }
    }

    async fn execute_file_tool(
        &mut self,
        name: &str,
        args: &Value,
        events: &mpsc::Sender<ProviderEvent>,
    ) -> String {
        if name == "read_files" {
            return self.read_files(args, events).await;
        }
        let path = args["path"].as_str().unwrap_or(".");
        if name == "read_file" {
            self.log_read(events, path, args).await;
        }
        if matches!(name, "edit_file" | "write_file") {
            if let Some(error) = self
                .check_edit_allowed(events, path, name == "write_file")
                .await
            {
                return error;
            }
            self.log_edit_target(events, path).await;
        }
        files::run_tool(
            &self.root,
            name,
            args,
            self.max_file_lines,
            &mut self.snapshots,
        )
    }

    async fn check_edit_allowed(
        &self,
        events: &mpsc::Sender<ProviderEvent>,
        path: &str,
        creating: bool,
    ) -> Option<String> {
        if self.permission == Permission::Read {
            let _ = emit_text(events, "log", format!("Từ chối: {path} (chế độ chỉ đọc)")).await;
            return Some("error: chế độ chỉ đọc — không được sửa hoặc ghi file".into());
        }
        if !self.scope.is_empty() && !in_scope(&self.root, &self.scope, path, creating) {
            let _ = emit_text(events, "log", format!("Từ chối: {path} (ngoài phạm vi được giao)")).await;
            return Some(format!("error: {path} is outside the files you were given ({}). Edit only those files; write_file may also create a new file next to them.", self.scope.join(", ")));
        }
        None
    }

    async fn log_edit_target(&self, events: &mpsc::Sender<ProviderEvent>, path: &str) {
        let message = match files::safe_path(&self.root, path) {
            Ok(_) => format!("Sửa {path}"),
            Err(error) => format!("Từ chối: {path} ({error})"),
        };
        let _ = emit_text(events, "log", message).await;
    }


    /// The "Đọc path a–b" line of one read (and the warning for a long file read without a range).
    async fn log_read(&self, events: &mpsc::Sender<ProviderEvent>, path: &str, args: &Value) {
        let Ok(file) = files::safe_path(&self.root, path) else { return };
        let Ok(text) = fs::read_to_string(file) else { return };
        let total = text.lines().count();
        let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
        if args["start_line"].is_null() && args["end_line"].is_null() && total > self.max_file_lines as usize {
            let _ = emit_text(events, "log", format!("Cảnh báo: {path} dài {total} dòng")).await;
        }
        let end = args["end_line"].as_u64().map(|value| value as usize).unwrap_or(total).min(total).min(start.saturating_add(399));
        let _ = emit_text(events, "log", format!("Đọc {path} {start}–{end}")).await;
    }

    /// Several reads in one tool call, so one step (one model round trip) can read everything the task needs.
    async fn read_files(&mut self, args: &Value, events: &mpsc::Sender<ProviderEvent>) -> String {
        const MAX_BATCH: usize = 8;
        let Some(list) = args["files"].as_array().filter(|list| !list.is_empty()) else {
            return "error: Thiếu files (danh sách {path, start_line, end_line})".into();
        };
        let mut parts = Vec::new();
        for entry in list.iter().take(MAX_BATCH) {
            let path = entry["path"].as_str().unwrap_or(".");
            self.log_read(events, path, entry).await;
            let text = files::run_tool(&self.root, "read_file", entry, self.max_file_lines, &mut self.snapshots);
            parts.push(format!("=== {path} ===\n{text}"));
        }
        if list.len() > MAX_BATCH {
            parts.push(format!("(only the first {MAX_BATCH} files were read: ask again for the others)"));
        }
        parts.join("\n\n")
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
mod tests;
