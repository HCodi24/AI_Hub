//! MutiS High chat: Codex coordinates, the app runs the DeepSeek worker and hands the real report back to Codex.
//!
//! Codex ends a turn with fenced `mutis-delegate` JSON blocks (see `core::rule::mutis_high_delegation`). The app runs
//! each block on the worker, then resumes the same Codex thread with the worker's report. The app, not Codex, counts
//! the attempts per task and tells Codex to take a task over once the limit is spent.
use super::{for_kind, ProviderEvent, ProviderRequest};
use crate::chat::{ChatMessage, Effort, Permission};
use crate::config::{MutisConfig, ProviderConfig};
use crate::core::text::truncate_chars;
use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub const MAX_ATTEMPTS: usize = 4;
/// Guard against a Codex that keeps delegating: the number of Codex turns in one chat reply.
const MAX_ROUNDS: usize = 12;
const FENCE: &str = "```mutis-delegate";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Delegation {
    pub title: String,
    #[serde(default)]
    pub files: Vec<String>,
    pub instructions: String,
    #[serde(default)]
    pub acceptance: Vec<String>,
}

/// Valid delegations in the text, plus a message for every block that did not parse.
pub fn extract(text: &str) -> (Vec<Delegation>, Vec<String>) {
    let (mut tasks, mut errors) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(start) = rest.find(FENCE) {
        let body = &rest[start + FENCE.len()..];
        let end = body.find("```").unwrap_or(body.len());
        match serde_json::from_str::<Delegation>(body[..end].trim()) {
            Ok(task) if !task.title.trim().is_empty() && !task.instructions.trim().is_empty() => tasks.push(task),
            Ok(_) => errors.push("a mutis-delegate block has an empty title or instructions".into()),
            Err(error) => errors.push(format!("a mutis-delegate block is not valid JSON ({error})")),
        }
        rest = &body[end..];
    }
    (tasks, errors)
}

/// Paths Codex named must be real before a worker run is spent: an existing file, or a new file in an existing folder.
pub fn check_files(folder: &str, files: &[String]) -> Vec<String> {
    if files.is_empty() {
        return vec!["the files list is empty: name the exact files".into()];
    }
    let root = std::path::Path::new(folder);
    files.iter().filter_map(|file| {
        let relative = file.replace('\\', "/");
        if relative.starts_with('/') || relative.as_bytes().get(1) == Some(&b':') || relative.split('/').any(|part| part == "..") {
            return Some(format!("`{file}` must be a relative path inside the project"));
        }
        let path = root.join(&relative);
        let parent_exists = path.parent().is_some_and(|parent| parent.is_dir());
        (!path.is_file() && !parent_exists).then(|| format!("`{file}` does not exist and neither does its folder"))
    }).collect()
}

const PRELOAD_MAX_LINES: usize = 150;

/// Current text of the small listed files, as `FILE:` blocks (the worker then skips those reads). Larger files, binary
/// files and files that contain a code fence stay for the worker to read by range.
fn preload(folder: &str, files: &[String]) -> String {
    files.iter().filter_map(|file| {
        let text = std::fs::read_to_string(std::path::Path::new(folder).join(file.replace('\\', "/"))).ok()?;
        (text.lines().count() <= PRELOAD_MAX_LINES && !text.contains("```")).then(|| format!("FILE: {file}\n{}\n```\n", text.trim_end()))
    }).collect()
}

fn worker_prompt(mutis: &MutisConfig, task: &Delegation, preloaded: &str) -> String {
    let mut parts = Vec::new();
    if mutis.enabled && !mutis.worker_rules.trim().is_empty() {
        parts.push(mutis.worker_rules.clone());
    }
    parts.push(format!("Task: {}", task.title.trim()));
    if !task.files.is_empty() {
        parts.push(format!("Files: {}", task.files.join(", ")));
    }
    parts.push(format!("Instructions:\n{}", task.instructions.trim()));
    if !task.acceptance.is_empty() {
        parts.push(format!("Acceptance criteria:\n{}", task.acceptance.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n")));
    }
    if !preloaded.is_empty() {
        parts.push(format!("Current content of the small listed files (already read for you; do not read them again):\n{preloaded}"));
    }
    parts.push("Make the edits yourself with the file tools; investigating without editing is a failed task. End with the files you changed and anything left undone.".into());
    parts.join("\n")
}

struct WorkerResult {
    summary: String,
    diff: String,
    error: Option<String>,
}

fn report(task: &Delegation, attempt: usize, result: &WorkerResult) -> String {
    let changed = if result.diff.trim().is_empty() {
        "NONE - the worker did not change any file".to_owned()
    } else {
        format!("yes, diff:\n{}", truncate_chars(&result.diff, 6000))
    };
    let status = result.error.as_deref().map_or("finished".to_owned(), |e| format!("ERROR: {e}"));
    format!(
        "DeepSeek worker report for \"{}\" (attempt {attempt}/{MAX_ATTEMPTS}):\nStatus: {status}\nFiles changed: {changed}\nWorker summary:\n{}\n\nReview this against your acceptance criteria. If it passes, say so and continue. If it fails, answer with another mutis-delegate block with the SAME title and concrete corrections ({} attempts left).",
        task.title.trim(),
        truncate_chars(&result.summary, 3000),
        MAX_ATTEMPTS - attempt,
    )
}

/// A new chronological turn: the browser and the saved record start a fresh assistant message for this provider.
fn turn(provider_kind: &str, model: &str) -> ProviderEvent {
    ProviderEvent {
        name: "turn",
        data: serde_json::json!({ "provider_kind": provider_kind, "model": model,
            "provider_name": if provider_kind == "deepseek" { "DeepSeek" } else { "Codex" } }),
    }
}

fn message(role: &str, content: String) -> ChatMessage {
    ChatMessage {
        debate_id: None, plan_id: None, role: role.into(), content, attachments: Vec::new(), reasoning: String::new(),
        logs: Vec::new(), diff: String::new(), files: Vec::new(), provider: String::new(), error: None,
    }
}

pub struct DelegateProvider {
    pub worker: ProviderConfig,
    pub model: String,
}

impl DelegateProvider {
    /// One Codex turn: every event goes to the chat; returns the reply text and the native session id.
    async fn codex_turn(&self, request: &ProviderRequest, events: &mpsc::Sender<ProviderEvent>, cancel: &CancellationToken) -> Result<(String, Option<String>)> {
        // The coordinator gets its own turn before the call, so its text never mixes with a worker turn.
        if cancel.is_cancelled() { return Ok((String::new(), None)); }
        if events.send(turn("codex_cli", &request.model)).await.is_err() {
            cancel.cancel();
            return Ok((String::new(), None));
        }
        let (sender, mut receiver) = mpsc::channel(32);
        let inner = for_kind("codex_cli")?;
        let (inner_request, inner_cancel) = (request.clone(), cancel.clone());
        let handle = tokio::spawn(async move { inner.run(inner_request, sender, inner_cancel).await });
        let (mut text, mut session) = (String::new(), None);
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => text.push_str(event.data["text"].as_str().unwrap_or("")),
                "session" => session = event.data["session_id"].as_str().map(str::to_owned),
                _ => {}
            }
            if events.send(event).await.is_err() {
                cancel.cancel();
                break;
            }
        }
        handle.await.map_err(|e| anyhow!(e))??;
        Ok((text, session))
    }

    async fn run_worker(&self, request: &ProviderRequest, task: &Delegation, attempt: usize, events: &mpsc::Sender<ProviderEvent>, cancel: &CancellationToken) -> WorkerResult {
        if cancel.is_cancelled() || events.send(turn("deepseek", &self.model)).await.is_err() {
            cancel.cancel();
            return WorkerResult { summary: String::new(), diff: String::new(), error: None };
        }
        let _ = events.send(ProviderEvent::text("log", format!("Giao DeepSeek: {} (lần {attempt}/{MAX_ATTEMPTS})", task.title.trim()))).await;
        let mut provider = self.worker.clone();
        provider.kind = "deepseek_agent".into();
        provider.agent_capable = true;
        let worker_request = ProviderRequest {
            provider, model: self.model.clone(), folder: request.folder.clone(), permission: Permission::Write, effort: Effort::Default,
            messages: vec![message("user", worker_prompt(&request.mutis, task, &preload(&request.folder, &task.files)))], session_id: None, mutis: request.mutis.clone(), mcp: Vec::new(),
            write_scope: task.files.clone(),
        };
        let (sender, mut receiver) = mpsc::channel(32);
        let (inner_cancel, run) = (cancel.clone(), for_kind("deepseek_agent"));
        let handle = tokio::spawn(async move {
            match run {
                Ok(provider) => provider.run(worker_request, sender, inner_cancel).await,
                Err(error) => Err(error),
            }
        });
        let mut result = WorkerResult { summary: String::new(), diff: String::new(), error: None };
        while let Some(event) = receiver.recv().await {
            match event.name {
                "delta" => {
                    result.summary.push_str(event.data["text"].as_str().unwrap_or(""));
                    let _ = events.send(event).await;
                }
                "reasoning" => {
                    let _ = events.send(event).await;
                }
                "diff" => {
                    result.diff.push_str(event.data["text"].as_str().unwrap_or(""));
                    let _ = events.send(event).await;
                }
                "log" => {
                    let line = event.data["text"].as_str().unwrap_or("");
                    let _ = events.send(ProviderEvent::text("log", format!("DeepSeek: {line}"))).await;
                }
                "error" => {
                    result.error = event.data["text"].as_str().map(str::to_owned);
                    // A worker failure stays visible in its own turn; Codex still reviews the report below.
                    let _ = events.send(event).await;
                }
                _ => {}
            }
        }
        let failure = match handle.await {
            Ok(Err(error)) => Some(error.to_string()),
            Err(error) => Some(error.to_string()),
            Ok(Ok(())) => None,
        };
        if let Some(error) = failure {
            if result.error.as_deref() != Some(error.as_str()) {
                let _ = events.send(ProviderEvent::text("error", error.clone())).await;
            }
            result.error = Some(error);
        }
        result
    }
}

#[async_trait::async_trait]
impl super::Provider for DelegateProvider {
    async fn run(&self, mut request: ProviderRequest, events: mpsc::Sender<ProviderEvent>, cancel: CancellationToken) -> Result<()> {
        let mut attempts: HashMap<String, usize> = HashMap::new();
        for _ in 0..MAX_ROUNDS {
            let (text, session) = self.codex_turn(&request, &events, &cancel).await?;
            if session.is_some() {
                request.session_id = session;
            }
            let (tasks, errors) = extract(&text);
            if cancel.is_cancelled() || (tasks.is_empty() && errors.is_empty()) {
                return Ok(());
            }
            let mut reply = errors.iter().map(|e| format!("Problem: {e}. Resend it as valid JSON.\n")).collect::<String>();
            for task in tasks {
                let used = attempts.entry(task.title.trim().to_lowercase()).or_insert(0);
                if *used >= MAX_ATTEMPTS {
                    reply.push_str(&format!("The attempt limit ({MAX_ATTEMPTS}/{MAX_ATTEMPTS}) for \"{}\" is spent. Implement it yourself now and verify the result.\n\n", task.title.trim()));
                    continue;
                }
                let problems = check_files(&request.folder, &task.files);
                if !problems.is_empty() {
                    reply.push_str(&format!("Path check failed for \"{}\": the worker was NOT run and this is not counted as an attempt. {}. Resend the block with correct paths.\n\n", task.title.trim(), problems.join("; ")));
                    continue;
                }
                *used += 1;
                let attempt = *used;
                let result = self.run_worker(&request, &task, attempt, &events, &cancel).await;
                if cancel.is_cancelled() {
                    return Ok(());
                }
                reply.push_str(&report(&task, attempt, &result));
                reply.push_str("\n\n");
            }
            request.messages.push(message("assistant", text));
            request.messages.push(message("user", reply));
        }
        let _ = events.send(ProviderEvent::text("log", format!("Dừng giao việc: quá {MAX_ROUNDS} vòng Codex trong một lượt"))).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_valid_blocks_and_reports_broken_ones() {
        let text = "Plan.\n```mutis-delegate\n{\"title\":\"A\",\"files\":[\"src/a.rs\"],\"instructions\":\"do a\",\"acceptance\":[\"x\"]}\n```\nand\n```mutis-delegate\n{not json}\n```\n```mutis-delegate\n{\"title\":\"B\",\"instructions\":\"do b\"}\n```";
        let (tasks, errors) = extract(text);
        assert_eq!(tasks.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(tasks[0].files, ["src/a.rs"]);
        assert_eq!(errors.len(), 1);
        assert!(extract("no blocks here").0.is_empty());
        assert_eq!(extract("```mutis-delegate\n{\"title\":\" \",\"instructions\":\"x\"}\n```").1.len(), 1);
    }

    #[test]
    fn report_flags_a_worker_that_changed_nothing() {
        let task = Delegation { title: "A".into(), files: vec![], instructions: "i".into(), acceptance: vec![] };
        let idle = WorkerResult { summary: "I only read files".into(), diff: String::new(), error: None };
        let text = report(&task, 2, &idle);
        assert!(text.contains("attempt 2/4") && text.contains("NONE") && text.contains("(2 attempts left)"));
        let edited = WorkerResult { summary: "done".into(), diff: "+ new line".into(), error: Some("boom".into()) };
        let text = report(&task, 4, &edited);
        assert!(text.contains("ERROR: boom") && text.contains("+ new line") && text.contains("(0 attempts left)"));
    }

    #[test]
    fn check_files_accepts_real_files_and_new_files_in_real_folders_only() {
        let root = std::env::temp_dir().join(format!("ai-hub-check-{}", std::process::id()));
        std::fs::create_dir_all(root.join("web")).unwrap();
        std::fs::write(root.join("web/a.ts"), "x").unwrap();
        let folder = root.to_str().unwrap();
        let ok = |files: &[&str]| check_files(folder, &files.iter().map(|f| f.to_string()).collect::<Vec<_>>()).is_empty();
        assert!(ok(&["web/a.ts", "web/new.ts"]));
        assert!(!ok(&["nope/new.ts"]) && !ok(&["../x.ts"]) && !ok(&["C:/x.ts"]) && !ok(&[]));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn worker_prompt_carries_the_task_files_and_criteria() {
        let task = Delegation { title: "T".into(), files: vec!["a.rs".into(), "b.rs".into()], instructions: "change x".into(), acceptance: vec!["builds".into()] };
        let prompt = worker_prompt(&MutisConfig::default(), &task, "FILE: a.rs\nfn a() {}\n```\n");
        assert!(prompt.contains("Task: T") && prompt.contains("Files: a.rs, b.rs") && prompt.contains("- builds"));
        assert!(prompt.contains("FILE: a.rs") && !worker_prompt(&MutisConfig::default(), &task, "").contains("already read"));
    }

    #[test]
    fn preload_keeps_small_plain_files_only() {
        let root = std::env::temp_dir().join(format!("ai-hub-preload-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("small.ts"), "const a = 1;\n").unwrap();
        std::fs::write(root.join("big.ts"), "x\n".repeat(PRELOAD_MAX_LINES + 1)).unwrap();
        std::fs::write(root.join("fenced.md"), "```\ncode\n```\n").unwrap();
        let files = ["small.ts", "big.ts", "fenced.md", "missing.ts"].map(String::from);
        let text = preload(root.to_str().unwrap(), &files);
        assert!(text.contains("FILE: small.ts\nconst a = 1;") && !text.contains("big.ts") && !text.contains("fenced.md") && !text.contains("missing"));
        assert_eq!(crate::providers::deepseek_agent::parse_edit_only(&text), vec![("small.ts".to_owned(), "const a = 1;".to_owned())]);
        std::fs::remove_dir_all(root).unwrap();
    }
}
