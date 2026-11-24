use super::parse::last_report;
use super::{lock, ModelUsage, TokenSnapshot, JS_DATE_MAX, JS_SAFE_MAX, MAX_POINTS};
use crate::features::chat::{valid_id, ChatMessage, ChatRecord};
use anyhow::{ensure, Context, Result};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

pub(super) fn history_path(directory: &Path, id: &str) -> Result<PathBuf> {
    ensure!(valid_id(id), "Invalid chat id");
    Ok(directory.join("token-usage").join(format!("{id}.jsonl")))
}

fn checked(left: u64, right: u64) -> Result<u64> {
    let sum = left.checked_add(right).context("Token totals overflow")?;
    ensure!(sum <= JS_SAFE_MAX, "Token totals exceed the safe integer range");
    Ok(sum)
}

/// Cumulative per-model totals of a chat: assistant messages only, grouped by the exact stored
/// model (empty = unknown, kept as its own group), in canonical model order.
pub fn chat_totals(messages: &[ChatMessage]) -> Result<Vec<ModelUsage>> {
    let mut groups: Vec<ModelUsage> = Vec::new();
    for message in messages.iter().filter(|message| message.role == "assistant") {
        let Some((input, output)) = last_report(&message.logs) else {
            continue;
        };
        let index = match groups.iter().position(|group| group.model == message.model) {
            Some(index) => index,
            None => {
                groups.push(ModelUsage {
                    model: message.model.clone(),
                    input: 0,
                    output: 0,
                    total: 0,
                });
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        group.input = checked(group.input, input)?;
        group.output = checked(group.output, output)?;
        group.total = checked(group.input, group.output)?;
    }
    groups.sort_by(|left, right| left.model.cmp(&right.model));
    Ok(groups)
}

struct Rows {
    snapshots: Vec<TokenSnapshot>,
    /// Byte length of the complete rows; anything after it is an unfinished trailing write.
    complete_len: u64,
}

fn read_rows(path: &Path) -> Result<Rows> {
    let bytes = fs::read(path).with_context(|| format!("Could not read {}", path.display()))?;
    let mut snapshots = Vec::new();
    let mut offset = 0usize;
    let mut complete_len = 0u64;
    let mut previous: Option<u64> = None;
    while offset < bytes.len() {
        let rest = &bytes[offset..];
        // No newline yet: the process died mid-write, so this row was never committed.
        let Some(newline) = rest.iter().position(|byte| *byte == b'\n') else {
            break;
        };
        let line = &rest[..newline];
        offset += newline + 1;
        complete_len = offset as u64;
        let text = std::str::from_utf8(line)
            .with_context(|| format!("Corrupt token history row in {}", path.display()))?;
        if text.trim().is_empty() {
            continue;
        }
        let mut snapshot: TokenSnapshot = serde_json::from_str(text)
            .with_context(|| format!("Corrupt token history row in {}", path.display()))?;
        snapshot
            .validate()
            .with_context(|| format!("Corrupt token history row in {}", path.display()))?;
        if let Some(previous) = previous {
            ensure!(
                snapshot.at > previous,
                "Token history rows are not in increasing time order in {}",
                path.display()
            );
        }
        previous = Some(snapshot.at);
        // Rows written by older builds may be ordered by total; readers always see canonical order.
        snapshot.canonicalize();
        snapshots.push(snapshot);
    }
    Ok(Rows {
        snapshots,
        complete_len,
    })
}

fn append(path: &Path, snapshot: &TokenSnapshot, complete_len: u64) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Could not create {}", parent.display()))?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(path)
        .with_context(|| format!("Could not open {}", path.display()))?;
    let len = file.metadata()?.len();
    if len > complete_len {
        // Drop the unfinished trailing write so the next row starts on a clean line.
        file.set_len(complete_len)
            .with_context(|| format!("Could not repair {}", path.display()))?;
    }
    file.seek(SeekFrom::End(0))?;
    let mut line = serde_json::to_string(snapshot).context("Could not encode token snapshot")?;
    line.push('\n');
    file.write_all(line.as_bytes())
        .with_context(|| format!("Could not append to {}", path.display()))?;
    Ok(())
}

/// Append the chat's current cumulative totals when they differ from the last stored snapshot.
/// Returns the new snapshot, or `None` when there is nothing to record.
pub fn record(directory: &Path, chat: &ChatRecord) -> Result<Option<TokenSnapshot>> {
    let models = chat_totals(&chat.messages)?;
    if models.is_empty() {
        return Ok(None);
    }
    let path = history_path(directory, &chat.id)?;
    let _guard = lock();
    let rows = if path.exists() {
        read_rows(&path)?
    } else {
        Rows {
            snapshots: Vec::new(),
            complete_len: 0,
        }
    };
    // Identical totals (also after a restart) are not a new observation.
    if rows
        .snapshots
        .last()
        .is_some_and(|last| last.models == models)
    {
        return Ok(None);
    }
    let mut at = crate::core::time::now_ms();
    if let Some(last) = rows.snapshots.last() {
        // A saturated timestamp would duplicate the previous row, which readers reject.
        at = at.max(
            last.at
                .checked_add(1)
                .context("Token history timestamp overflow")?,
        );
    }
    ensure!(
        at <= JS_DATE_MAX,
        "Token history timestamp is outside the JavaScript Date range"
    );
    let mut snapshot = TokenSnapshot { at, models };
    snapshot.canonicalize();
    snapshot.validate()?;
    append(&path, &snapshot, rows.complete_len)?;
    Ok(Some(snapshot))
}

/// The latest `MAX_POINTS` validated snapshots of one chat; a missing file is simply empty.
pub fn history(directory: &Path, id: &str) -> Result<Vec<TokenSnapshot>> {
    let path = history_path(directory, id)?;
    let _guard = lock();
    if !path.exists() {
        return Ok(Vec::new());
    }
    let rows = read_rows(&path)?;
    let start = rows.snapshots.len().saturating_sub(MAX_POINTS);
    Ok(rows.snapshots[start..].to_vec())
}
