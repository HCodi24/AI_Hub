use super::super::events::{persist_event, should_emit_empty_reply};
use crate::{
    app::AppState,
    chat::{self, ChatRecord},
    features::{
        changes::{FileChange, Snapshot},
        memory::MemoryConfig,
    },
    mutis::Event,
    providers::ProviderEvent,
};
use serde_json::{json, Value};
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type SseSender = mpsc::Sender<axum::response::sse::Event>;

pub(super) async fn finish(
    record: &mut ChatRecord,
    sse_sender: &SseSender,
    task_state: &AppState,
    task_cancel: &CancellationToken,
    received_delta: bool,
    received_error: bool,
    last_persist: &mut Instant,
    delegated: bool,
    first_turn: usize,
    files_before: &Snapshot,
    memory_config: &MemoryConfig,
    chat_id: &str,
    request_id: Uuid,
    task_snapshot: Option<Vec<Value>>,
    log_enabled: bool,
    started: Instant,
    task_kind: Option<String>,
    make_event: impl Fn(&str, Option<String>, bool, Option<u64>, Option<u64>, Option<String>) -> Event,
) {
    // Stopped by the user or dropped by the browser: save that on the reply so a reload does not show it as complete.
    if task_cancel.is_cancelled() && !received_error {
        if let Some(message) = record
            .messages
            .last_mut()
            .filter(|m| m.role == "assistant" && m.error.is_none())
        {
            message.error = Some(chat::STOPPED_NOTE.to_owned());
        }
    }
    if should_emit_empty_reply(received_delta, received_error, task_cancel.is_cancelled()) {
        let event = ProviderEvent::text(
            "error",
            "AI kết thúc mà không trả lời — có thể bị ngắt giữa chừng. Hãy gửi lại.",
        );
        persist_event(&task_state.chats, record, &event, last_persist);
        let _ = sse_sender.send(chat::sse_event("error", &event.data)).await;
    }
    let files = finish_files(record, sse_sender, files_before).await;
    finish_task_snapshot(record, sse_sender, task_snapshot).await;
    let memory_logs = apply_memory(
        record,
        sse_sender,
        task_state,
        memory_config,
        chat_id,
        first_turn,
        delegated,
        &files,
    )
    .await;
    if let Some(message) = record.messages.last_mut() {
        message.logs.extend(memory_logs);
    }
    if let Err(error) = chat::save_record(&task_state.chats, record) {
        eprintln!("Could not save chat record: {error:#}");
    }
    if log_enabled {
        log_task_end(record, task_state, started, task_kind, make_event);
    }
    let _ = sse_sender
        .send(chat::sse_event("done", &json!({ "ok": true })))
        .await;
    let mut running = task_state.running.lock().await;
    if running
        .get(chat_id)
        .is_some_and(|entry| entry.request_id == request_id)
    {
        running.remove(chat_id);
    }
}

async fn finish_files(
    record: &mut ChatRecord,
    sse_sender: &SseSender,
    files_before: &Snapshot,
) -> Vec<FileChange> {
    let files_after = if record.folder.is_empty() {
        Default::default()
    } else {
        crate::features::changes::snapshot(std::path::Path::new(&record.folder)).await
    };
    let files = crate::features::changes::changes(files_before, &files_after);
    if !files.is_empty() {
        // A source that sent no diff of its own still gets one, from git.
        if record.messages.last().is_some_and(|message| message.diff.trim().is_empty()) {
            let diff =
                crate::features::changes::diff_of(std::path::Path::new(&record.folder), &files)
                    .await;
            if !diff.is_empty() {
                let _ = sse_sender.send(chat::sse_event("diff", &json!({ "text": diff }))).await;
                if let Some(message) = record.messages.last_mut() {
                    message.diff = diff;
                }
            }
        }
        let _ = sse_sender
            .send(chat::sse_event("files", &json!({ "files": files })))
            .await;
        if let Some(message) = record.messages.last_mut() {
            message.files = files.clone();
        }
    }
    files
}

async fn finish_task_snapshot(
    record: &mut ChatRecord,
    sse_sender: &SseSender,
    task_snapshot: Option<Vec<Value>>,
) {
    if let Some(tasks) = task_snapshot {
        let tasks: Vec<Value> = tasks
            .into_iter()
            .map(|mut task| {
                let unfinished =
                    task.get("state")
                        .and_then(Value::as_str)
                        .is_some_and(|state| {
                            matches!(state, "waiting" | "coding" | "checking" | "fixwait" | "fixing")
                        });
                if unfinished {
                    if let Some(fields) = task.as_object_mut() {
                        fields.insert("state".to_owned(), Value::String("stopped".to_owned()));
                    }
                }
                task
            })
            .collect();
        if let Some(message) = record
            .messages
            .iter_mut()
            .rev()
            .find(|message| message.role == "assistant")
        {
            message.tasks = Some(tasks.clone());
            let _ = sse_sender
                .send(chat::sse_event("task_snapshot", &json!({ "tasks": tasks })))
                .await;
        }
    }
}

async fn apply_memory(
    record: &mut ChatRecord,
    sse_sender: &SseSender,
    task_state: &AppState,
    memory_config: &MemoryConfig,
    chat_id: &str,
    first_turn: usize,
    delegated: bool,
    files: &[FileChange],
) -> Vec<String> {
    let mut memory_logs = Vec::new();
    for message in record
        .messages
        .iter_mut()
        .skip(first_turn)
        .filter(|m| m.role == "assistant")
    {
        let (clean, block) = crate::features::memory::strip_blocks(&message.content);
        message.content = clean;
        if memory_config.enabled && !(delegated && message.provider == "deepseek") {
            if let Some(block) = block.filter(|b| b.trim() != "none") {
                match serde_json::from_str::<crate::features::memory::Update>(&block) {
                    Ok(update) => {
                        match crate::features::memory::Store::open(
                            &task_state.config_path,
                            &record.folder,
                            memory_config,
                        ) {
                            Ok(store) => {
                                let paths = files.iter().map(|f| f.path.clone()).collect();
                                match store.merge(update, chat_id, paths) {
                                    Ok(change) => {
                                        let value = json!(change);
                                        memory_logs.push(
                                            ["scope", "status", "action"]
                                                .iter()
                                                .filter_map(|key| value[*key].as_str())
                                                .collect::<Vec<_>>()
                                                .join(" · "),
                                        );
                                        let _ = sse_sender
                                            .send(chat::sse_event("memory", &value))
                                            .await;
                                    }
                                    Err(error) => {
                                        eprintln!("Could not merge chat memory: {error:#}")
                                    }
                                }
                            }
                            Err(error) => eprintln!("Could not open chat memory: {error:#}"),
                        }
                    }
                    Err(_) => {
                        let text = "Bộ nhớ: khối <memory> không hợp lệ";
                        // The notification is emitted on the final active turn in both clients.
                        memory_logs.push(text.to_owned());
                        let _ = sse_sender
                            .send(chat::sse_event("log", &json!({"text":text})))
                            .await;
                    }
                }
            }
        }
    }
    memory_logs
}

fn log_task_end(
    record: &ChatRecord,
    task_state: &AppState,
    started: Instant,
    task_kind: Option<String>,
    make_event: impl Fn(&str, Option<String>, bool, Option<u64>, Option<u64>, Option<String>) -> Event,
) {
    let token_line = record
        .messages
        .last()
        .and_then(|m| m.logs.iter().rev().find(|line| line.starts_with("Token:")));
    let tokens = token_line.and_then(|line| {
        line.split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .filter_map(|part| part.parse::<u64>().ok())
            .take(2)
            .reduce(u64::saturating_add)
    });
    let ok = record.messages.last().is_some_and(|m| m.error.is_none());
    let _ = task_state.event_log.append(&make_event(
        "task_end",
        task_kind,
        ok,
        Some(started.elapsed().as_millis().min(u64::MAX as u128) as u64),
        tokens,
        None,
    ));
}
