use crate::providers::ProviderRequest;
use anyhow::Result;
use serde_json::{json, Value};
use std::path::Path;

const SYSTEM_WRITE: &str = concat!(
    "You are a coding assistant with file tools. Work only in the supplied folder with the tools. ",
    "Do not run commands. Answer the user's request; make only the changes they asked for, ",
    "then give a concise report."
);
const SYSTEM_FULL: &str = concat!(
    "You are a coding assistant with file tools. Work only in the supplied folder with the tools. ",
    "You can run commands in the working folder with run_command; prefer small, safe commands; ",
    "never run destructive commands the user did not ask for. Answer the user's request; make only the changes they asked for, ",
    "then give a concise report."
);
const SYSTEM_MCP: &str = concat!(
    "You are an assistant with tools provided by MCP servers. Use them when they help; ",
    "you cannot read or edit files or run commands except through those tools. Answer concisely."
);
const SYSTEM_READ: &str = concat!(
    "You are a coding assistant with READ-ONLY file tools (list and read). ",
    "Work only in the supplied folder. You cannot edit files or run commands. ",
    "Explore the folder as needed and answer the user's question concisely."
);

pub fn parse_edit_only(input: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut lines = input.lines().peekable();
    while let Some(line) = lines.next() {
        let heading = line.trim().trim_matches('`').trim();
        if let Some(header) = heading.strip_prefix("FILE: ") {
            let path = header
                .split_once(" LINES ")
                .map(|(path, lines)| format!("{path} (lines {lines})"))
                .unwrap_or_else(|| header.to_owned());
            let mut content = String::new();
            for next in lines.by_ref() {
                if next.trim().starts_with("```") {
                    break;
                }
                content.push_str(next);
                content.push('\n');
            }
            out.push((path.trim().to_owned(), content.trim_end().to_owned()));
        }
    }
    out
}

/// The answer part of a reply: leaked tool-call markup (`<｜DSML｜function_calls>...`) is not an answer.
pub(super) fn strip_dsml(content: &str) -> &str {
    content.find("DSML").map_or(content, |at| &content[..at]).trim_end_matches(|c: char| c == '<' || c == '｜' || c.is_whitespace()).trim_start()
}

pub(super) fn system_prompt(request: &ProviderRequest, files: bool) -> String {
    let system = if !files {
        SYSTEM_MCP
    } else {
        match request.permission {
            crate::providers::Permission::Read => SYSTEM_READ,
            crate::providers::Permission::Write => SYSTEM_WRITE,
            crate::providers::Permission::Full => SYSTEM_FULL,
        }
    };
    let mut system = system.to_string();
    if files {
        system.push_str(&format!("\n\n{}", crate::core::rule::deepseek::RULE));
        if crate::providers::is_deepseek_flash(&request.model) {
            system.push_str(&format!("\n\n{}", crate::core::rule::deepseek_flash::RULE));
        }
    }
    system
}

pub(super) fn build_messages(request: &ProviderRequest, root: &Path, task: &str, excerpts: Vec<(String, String)>, system: String) -> Result<Vec<Value>> {
    let mut messages = vec![json!({"role":"system","content":system})];
    if excerpts.is_empty() {
        let last = request.messages.len().saturating_sub(1);
        for (index, m) in request.messages.iter().enumerate() {
            // Only the newest message carries its images (if the model has no vision the API reports it).
            messages.push(if index == last {
                crate::providers::openai_compat::openai_message(m)?
            } else {
                json!({"role":m.role,"content":m.content})
            });
        }
    } else {
        let history = request
            .messages
            .iter()
            .filter(|m| m.role != "user" || m.content != task)
            .map(|m| json!({"role":m.role,"content":m.content}));
        messages.extend(history);
        let mut prompt = format!(
            "Working directory: {}\n\nTask:\n{}\n\nPreloaded file excerpts (FILE blocks):\n",
            root.display(),
            task
        );
        for (path, lines) in excerpts {
            prompt.push_str(&format!("\n--- {path} ---\n{lines}\n"));
        }
        messages.push(json!({"role":"user","content":prompt}));
    }
    Ok(messages)
}
