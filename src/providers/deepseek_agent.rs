use super::{emit_text, Provider, ProviderEvent, ProviderRequest};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const SYSTEM: &str = "You are a file editing agent. Work only in the supplied folder with the tools. Do not run commands. Make only the requested changes, then give a concise report.";

#[async_trait]
impl Provider for DeepSeekAgentProvider {
    async fn run(
        &self,
        request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if !request.mutis.effective(request.mutis.deepseek_agent) {
            bail!("Bật MutiS › DeepSeek làm việc trên file để dùng nguồn này");
        }
        if request.provider.api_key.trim().is_empty() {
            bail!("API key chưa được cấu hình cho nhà cung cấp này");
        }
        let base = request.provider.base_url.trim().trim_end_matches('/');
        if base.is_empty() {
            bail!("URL cơ sở chưa được cấu hình cho nhà cung cấp này");
        }
        let root = PathBuf::from(&request.folder)
            .canonicalize()
            .context("Thư mục làm việc không tồn tại")?;
        if !root.is_dir() {
            bail!("Thư mục làm việc không hợp lệ");
        }
        let task = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or_default();
        let excerpts = parse_edit_only(task);
        let edits_requested = task_asks_edits(task);
        let mut messages = vec![json!({"role":"system","content":SYSTEM})];
        if excerpts.is_empty() {
            messages.extend(
                request
                    .messages
                    .iter()
                    .map(|m| json!({"role":m.role,"content":m.content})),
            );
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
        let tools = tool_definitions();
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;
        let mut snapshots = BTreeMap::<String, Option<String>>::new();
        let mut steps = 0;
        let mut tokens_in = 0u64;
        let mut tokens_out = 0u64;
        let mut had_tool = false;
        let mut final_text = String::new();
        let mut failure: Option<anyhow::Error> = None;
        for step in 0..request.mutis.deepseek_max_steps.clamp(5, 40) {
            if cancel.is_cancelled() {
                failure = Some(anyhow::anyhow!("Đã dừng"));
                break;
            }
            steps = step + 1;
            let body = json!({"model":request.model,"messages":messages,"tools":tools,"max_tokens":8192,"temperature":0.2,"stream":false});
            let response = super::send_with_retry(
                &client,
                &format!("{base}/chat/completions"),
                vec![(
                    "Authorization".into(),
                    format!("Bearer {}", request.provider.api_key),
                )],
                &body,
                &request.mutis,
                &events,
                &cancel,
            )
            .await;
            let response = match response {
                Ok(v) => v,
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            };
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                failure = Some(anyhow::anyhow!(
                    "DeepSeek API HTTP {status}: {}",
                    super::safe_error_body(&body, &request.provider.api_key)
                ));
                break;
            }
            let decoded = tokio::select! {
                _ = cancel.cancelled() => { failure = Some(anyhow::anyhow!("Đã dừng")); break; }
                result = response.json::<Value>() => result,
            };
            let value: Value = match decoded {
                Ok(v) => v,
                Err(e) => {
                    failure = Some(e.into());
                    break;
                }
            };
            tokens_in =
                tokens_in.saturating_add(value["usage"]["prompt_tokens"].as_u64().unwrap_or(0));
            tokens_out = tokens_out
                .saturating_add(value["usage"]["completion_tokens"].as_u64().unwrap_or(0));
            let msg = &value["choices"][0]["message"];
            if msg.is_null() {
                failure = Some(anyhow::anyhow!(
                    "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                ));
                break;
            }
            let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
            let content = msg["content"].as_str().unwrap_or_default();
            if calls.is_empty() {
                final_text = content.trim().to_owned();
                if final_text.is_empty() || (edits_requested && !had_tool) {
                    failure = Some(anyhow::anyhow!(
                        "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
                    ));
                }
                break;
            }
            had_tool = true;
            messages.push(json!({"role":"assistant","content":if content.is_empty(){Value::Null}else{json!(content)},"tool_calls":calls}));
            for call in calls {
                if cancel.is_cancelled() {
                    failure = Some(anyhow::anyhow!("Đã dừng"));
                    break;
                }
                let id = call["id"].as_str().unwrap_or_default();
                let name = call["function"]["name"].as_str().unwrap_or_default();
                let args = serde_json::from_str::<Value>(
                    call["function"]["arguments"].as_str().unwrap_or("{}"),
                )
                .unwrap_or(json!({}));
                let path = args["path"].as_str().unwrap_or(".");
                if name == "read_file" {
                    if let Ok(file) = safe_path(&root, path) {
                        if let Ok(text) = fs::read_to_string(file) {
                            let total = text.lines().count();
                            let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
                            if args["start_line"].is_null()
                                && args["end_line"].is_null()
                                && total > request.mutis.deepseek_max_file_lines as usize
                            {
                                let _ = emit_text(
                                    &events,
                                    "log",
                                    format!("Cảnh báo: {path} dài {total} dòng"),
                                )
                                .await;
                            }
                            let end = args["end_line"]
                                .as_u64()
                                .map(|v| v as usize)
                                .unwrap_or(total)
                                .min(total)
                                .min(start.saturating_add(399));
                            let _ = emit_text(&events, "log", format!("Đọc {path} {start}–{end}"))
                                .await;
                        }
                    }
                }
                if matches!(name, "edit_file" | "write_file") {
                    match safe_path(&root, path) {
                        Ok(_) => {
                            let _ = emit_text(&events, "log", format!("Sửa {path}")).await;
                        }
                        Err(error) => {
                            let _ = emit_text(&events, "log", format!("Từ chối: {path} ({error})"))
                                .await;
                        }
                    }
                }
                let result = run_tool(
                    &root,
                    name,
                    &args,
                    request.mutis.deepseek_max_file_lines,
                    &mut snapshots,
                );
                messages.push(json!({"role":"tool","tool_call_id":id,"content":result}));
            }
            if failure.is_some() {
                break;
            }
        }
        if failure.is_none() && final_text.is_empty() {
            failure = Some(anyhow::anyhow!(
                "DeepSeek trả về rỗng (có thể chạm giới hạn đầu ra) — hãy chia nhỏ việc"
            ));
        }
        if !final_text.is_empty() {
            emit_text(&events, "delta", final_text).await?;
        }
        if tokens_in + tokens_out > 0 {
            emit_text(
                &events,
                "log",
                format!("Token: vào {tokens_in} (cache 0), ra {tokens_out} (suy luận 0)"),
            )
            .await?;
        }
        let diff = make_diff(&root, &snapshots);
        for (path, before) in &snapshots {
            let after = fs::read_to_string(root.join(path)).ok();
            let (added, removed) = line_counts(
                before.as_deref().unwrap_or(""),
                after.as_deref().unwrap_or(""),
            );
            emit_text(
                &events,
                "log",
                format!("Đã sửa {path} (+{added} −{removed} dòng)"),
            )
            .await?;
        }
        events
            .send(ProviderEvent {
                name: "diff",
                data: json!({"text":diff}),
            })
            .await
            .map_err(|_| anyhow::anyhow!("Chat stream closed"))?;
        if let Some(error) = failure {
            return Err(error);
        }
        let _ = steps;
        Ok(())
    }
}

pub struct DeepSeekAgentProvider;

fn tool_definitions() -> Value {
    let s = json!({"type":"string"});
    json!([
        {
            "type": "function",
            "function": {
                "name": "list_dir",
                "description": "List up to 300 entries in a relative directory.",
                "parameters": {
                    "type": "object",
                    "properties": {"path": s},
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": concat!(
                    "Read numbered lines from a text file. At most 400 lines; ",
                    "use ranges for long files."
                ),
                "parameters": {
                    "type": "object",
                    "properties": {
                        "path": s,
                        "start_line": {"type": "integer"},
                        "end_line": {"type": "integer"}
                    },
                    "required": ["path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "edit_file",
                "description": "Replace one exact unique occurrence.",
                "parameters": {
                    "type": "object",
                    "properties": {"path": s, "old_text": s, "new_text": s},
                    "required": ["path", "old_text", "new_text"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Create or overwrite a UTF-8 file.",
                "parameters": {
                    "type": "object",
                    "properties": {"path": s, "content": s},
                    "required": ["path", "content"]
                }
            }
        }
    ])
}

fn safe_path(root: &Path, raw: &str) -> Result<PathBuf> {
    if raw.starts_with('/') || raw.starts_with('\\') || raw.as_bytes().get(1) == Some(&b':') {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let normalized = raw.replace('\\', "/");
    if normalized.split('/').any(|part| part == "..") {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let rel = Path::new(&normalized);
    if rel.as_os_str().is_empty()
        || rel.is_absolute()
        || rel
            .components()
            .any(|c| !matches!(c, Component::Normal(_) | Component::CurDir))
    {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    if rel.components().any(|c| {
        c.as_os_str().to_string_lossy().eq_ignore_ascii_case(".git")
            || c.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("target")
            || c.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("node_modules")
            || c.as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(".codegraph")
    }) {
        bail!("Đường dẫn thuộc vùng bị cấm");
    }
    let candidate = root.join(rel);
    let mut ancestor = candidate.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .context("Đường dẫn không hợp lệ")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().context("Đường dẫn không hợp lệ")?;
    }
    let mut check = ancestor.canonicalize()?;
    for part in suffix.iter().rev() {
        check.push(part);
    }
    if check != root && !check.starts_with(root) {
        bail!("Đường dẫn vượt khỏi thư mục chat");
    }
    Ok(candidate)
}

fn run_tool(
    root: &Path,
    name: &str,
    args: &Value,
    max_lines: u32,
    snapshots: &mut BTreeMap<String, Option<String>>,
) -> String {
    let result = (|| -> Result<String> {
        let raw = args["path"].as_str().context("Thiếu path")?;
        let path = safe_path(root, raw)?;
        let relative = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        match name {
            "list_dir" => {
                let mut entries = fs::read_dir(&path)?
                    .filter_map(Result::ok)
                    .map(|e| {
                        let typ = if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            "d"
                        } else {
                            "f"
                        };
                        format!("{typ} {}", e.file_name().to_string_lossy())
                    })
                    .collect::<Vec<_>>();
                entries.sort();
                entries.truncate(300);
                Ok(entries.join("\n"))
            }
            "read_file" => {
                let text = fs::read_to_string(&path)?;
                let lines = text.lines().collect::<Vec<_>>();
                let start = args["start_line"].as_u64().unwrap_or(1).max(1) as usize;
                let end = args["end_line"].as_u64().unwrap_or(lines.len() as u64) as usize;
                if args["start_line"].is_null()
                    && args["end_line"].is_null()
                    && lines.len() > max_lines as usize
                {
                    bail!("File dài {} dòng, hãy đọc theo khoảng start_line/end_line (tối đa 400 dòng mỗi lần)",lines.len());
                }
                if end < start {
                    bail!("end_line phải >= start_line");
                }
                let end = end.min(lines.len()).min(start.saturating_add(399));
                let result = lines
                    .iter()
                    .enumerate()
                    .skip(start - 1)
                    .take(end.saturating_sub(start) + 1)
                    .map(|(i, l)| format!("{}: {}", i + 1, l))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(result)
            }
            "edit_file" => {
                let text = fs::read_to_string(&path)?;
                let old = args["old_text"].as_str().context("Thiếu old_text")?;
                let new = args["new_text"].as_str().context("Thiếu new_text")?;
                let updated = exact_replace(&text, old, new)?;
                snapshots.entry(relative.clone()).or_insert(Some(text));
                fs::write(&path, updated)?;
                Ok("edited".into())
            }
            "write_file" => {
                let content = args["content"].as_str().context("Thiếu content")?;
                snapshots
                    .entry(relative.clone())
                    .or_insert_with(|| fs::read_to_string(&path).ok());
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, content)?;
                Ok(format!("wrote {} chars", content.chars().count()))
            }
            _ => bail!("Công cụ không tồn tại"),
        }
    })();
    result.unwrap_or_else(|e| format!("error: {e:#}"))
}

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
fn task_asks_edits(task: &str) -> bool {
    let text = task.to_lowercase();
    [
        "edit",
        "modify",
        "change",
        "write",
        "create",
        "fix",
        "implement",
        "sửa",
        "chỉnh",
        "ghi",
        "tạo",
        "thêm",
        "xóa",
        "xoá",
        "cập nhật",
    ]
    .iter()
    .any(|word| text.contains(word))
}
fn exact_replace(text: &str, old: &str, new: &str) -> Result<String> {
    let count = text.matches(old).count();
    if count != 1 {
        bail!("old_text khớp {count} lần; cần đúng 1 lần");
    }
    Ok(text.replacen(old, new, 1))
}
fn line_counts(before: &str, after: &str) -> (usize, usize) {
    fn frequencies(text: &str) -> BTreeMap<&str, usize> {
        let mut counts = BTreeMap::new();
        for line in text.lines() {
            *counts.entry(line).or_insert(0) += 1;
        }
        counts
    }
    let old = frequencies(before);
    let new = frequencies(after);
    let added = new
        .iter()
        .map(|(line, count)| count.saturating_sub(old.get(line).copied().unwrap_or(0)))
        .sum();
    let removed = old
        .iter()
        .map(|(line, count)| count.saturating_sub(new.get(line).copied().unwrap_or(0)))
        .sum();
    (added, removed)
}
fn make_diff(root: &Path, snapshots: &BTreeMap<String, Option<String>>) -> String {
    let mut out = String::new();
    for (path, before) in snapshots {
        let after = fs::read_to_string(root.join(path)).unwrap_or_default();
        out.push_str(&format!("--- a/{path}\n+++ b/{path}\n"));
        let old = before.as_deref().unwrap_or("").lines().collect::<Vec<_>>();
        let new = after.lines().collect::<Vec<_>>();
        out.push_str(&format!("@@ -1,{} +1,{} @@\n", old.len(), new.len()));
        for line in &old {
            out.push('-');
            out.push_str(line);
            out.push('\n');
        }
        for line in &new {
            out.push('+');
            out.push_str(line);
            out.push('\n');
        }
        if out.len() > 20 * 1024 {
            out.truncate(20 * 1024);
            out.push_str("\n...[diff truncated at 20 KB]\n");
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_paths() {
        let root = std::env::current_dir().unwrap();
        for path in [
            "../escape",
            "C:\\temp\\x",
            ".git/config",
            "target/out",
            "node_modules/x",
            ".codegraph/index",
        ] {
            assert!(safe_path(&root, path).is_err(), "{path}");
        }
    }
    #[test]
    fn edit_only_blocks_are_parsed() {
        assert_eq!(
            parse_edit_only("Do this\n```FILE: src/lib.rs LINES 1-2\nlet x = 1;\n```"),
            vec![("src/lib.rs (lines 1-2)".to_owned(), "let x = 1;".to_owned())]
        );
    }
    #[test]
    fn recognizes_edit_requests() {
        assert!(task_asks_edits("Sửa hàm này"));
        assert!(!task_asks_edits("Giải thích hàm này"));
    }
    #[test]
    fn edit_requires_exactly_one_match() {
        assert_eq!(
            exact_replace("a b a", "a", "x").unwrap_err().to_string(),
            "old_text khớp 2 lần; cần đúng 1 lần"
        );
        assert_eq!(exact_replace("a b", "a", "x").unwrap(), "x b");
    }
    #[test]
    fn diff_counts_changed_lines() {
        assert_eq!(line_counts("a\nb\n", "a\nc\n"), (1, 1));
    }
}
