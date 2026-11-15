//! Per-chat token history, independent of memory and of the Dashboard.
//!
//! Every accepted token log line of an assistant message is aggregated into cumulative per-model
//! totals (the same "last valid report per message" rule the chat UI uses) and appended as one
//! JSONL snapshot at `<chats>/token-usage/<chat uuid>.jsonl`. Only counts and the model name are
//! stored: never content, prompts or memory data.
use crate::features::chat::{valid_id, ChatMessage, ChatRecord};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

/// Largest integer JavaScript can hold exactly; anything above it is rejected instead of rounded.
pub const JS_SAFE_MAX: u64 = 9_007_199_254_740_991;
/// Largest timestamp a JavaScript `Date` can represent; later values are not plottable.
pub const JS_DATE_MAX: u64 = 8_640_000_000_000_000;
/// How many points a reader gets back; the file itself keeps every snapshot.
pub const MAX_POINTS: usize = 500;

/// Serializes every read/append of the history files so two chats (or two runs) never interleave.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// One model's cumulative totals inside a snapshot.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub model: String,
    pub input: u64,
    pub output: u64,
    pub total: u64,
}

/// One stored observation: `{at: milliseconds, models: [...]}`, models in canonical order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenSnapshot {
    pub at: u64,
    pub models: Vec<ModelUsage>,
}

impl TokenSnapshot {
    /// A stored row must be self-consistent before it is trusted.
    fn validate(&self) -> Result<()> {
        ensure!(!self.models.is_empty(), "Token history row has no models");
        ensure!(
            self.at <= JS_DATE_MAX,
            "Token history row has a timestamp outside the JavaScript Date range"
        );
        for (index, model) in self.models.iter().enumerate() {
            ensure!(
                model.input <= JS_SAFE_MAX && model.output <= JS_SAFE_MAX,
                "Token history row has counts beyond the safe integer range"
            );
            ensure!(
                model.total == model.input + model.output && model.total <= JS_SAFE_MAX,
                "Token history row has an inconsistent total"
            );
            ensure!(
                !self.models[..index]
                    .iter()
                    .any(|other| other.model == model.model),
                "Token history row repeats a model identity"
            );
        }
        Ok(())
    }

    /// Canonical order: exact model identity, so equal totals never reorder rows.
    fn canonicalize(&mut self) {
        self.models.sort_by(|left, right| left.model.cmp(&right.model));
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    STORE_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}

fn history_path(directory: &Path, id: &str) -> Result<PathBuf> {
    ensure!(valid_id(id), "Invalid chat id");
    Ok(directory.join("token-usage").join(format!("{id}.jsonl")))
}

fn same_char_ci(left: char, right: char) -> bool {
    left == right || left.to_lowercase().eq(right.to_lowercase())
}

/// Case-insensitive prefix match that also folds non-ASCII letters (`VÀO` == `vào`).
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let mut offset = 0usize;
    for expected in prefix.chars() {
        let actual = text[offset..].chars().next()?;
        if !same_char_ci(actual, expected) {
            return None;
        }
        offset += actual.len_utf8();
    }
    Some(&text[offset..])
}

/// The `\s+` the frontend requires between a keyword and its number.
fn require_space(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    (trimmed.len() < text.len()).then_some(trimmed)
}

/// `\d+` with overflow and safe-integer rejection.
fn digits(text: &str) -> Option<(u64, &str)> {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let value: u64 = text[..end].parse().ok()?;
    (value <= JS_SAFE_MAX).then(|| (value, &text[end..]))
}

/// The optional `(...)` metadata group (cache, reasoning, ...): never added to the counts.
fn skip_paren(text: &str) -> &str {
    if let Some(inner) = text.strip_prefix('(') {
        if let Some(close) = inner.find(')') {
            return &inner[close + 1..];
        }
    }
    text
}

/// The optional `[\d.,]+` cost tail.
fn cost(text: &str) -> Option<&str> {
    let end = text
        .find(|character: char| !(character.is_ascii_digit() || character == '.' || character == ','))
        .unwrap_or(text.len());
    (end > 0).then(|| &text[end..])
}

/// Parse one log line exactly like `web/src/lib/chat-token-usage.ts`:
/// `Token: vào 1200 (cache 300), ra 45 (suy luận 10)` with an optional `, chi phí $0.12` tail.
/// The keywords are case-insensitive (including Vietnamese letters) and the whitespace after
/// `vào`/`ra` is required, exactly as in the frontend `REPORT` expression.
pub fn parse_report(log: &str) -> Option<(u64, u64)> {
    let rest = strip_prefix_ci(log.trim_start(), "token:")?;
    let rest = strip_prefix_ci(rest.trim_start(), "vào")?;
    let (input, rest) = digits(require_space(rest)?)?;
    let rest = skip_paren(rest.trim_start()).trim_start();
    let rest = rest.strip_prefix(',')?.trim_start();
    let rest = strip_prefix_ci(rest, "ra")?;
    let (output, rest) = digits(require_space(rest)?)?;
    let rest = skip_paren(rest.trim_start()).trim_start();
    let rest = if rest.is_empty() {
        rest
    } else {
        let rest = rest.strip_prefix(',')?.trim_start();
        let rest = strip_prefix_ci(rest, "chi phí")?.trim_start();
        let rest = rest.strip_prefix('$').unwrap_or(rest).trim_start();
        cost(rest)?.trim_start()
    };
    rest.is_empty().then_some((input, output))
}

/// True when this log line is an accepted token report, i.e. worth an observation.
pub fn is_token_report(log: &str) -> bool {
    parse_report(log).is_some()
}

/// The last valid report of one message, matching the summary the chat UI already shows.
pub fn last_report(logs: &[String]) -> Option<(u64, u64)> {
    logs.iter().rev().find_map(|log| parse_report(log))
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
    let mut at = now_ms();
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

#[cfg(test)]
#[path = "token_usage_tests.rs"]
mod tests;

