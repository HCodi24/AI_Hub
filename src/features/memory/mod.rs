//! Per-project, per-chat notes; no provider calls live here.
use crate::core::time::{civil, now_secs, Civil};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

mod core_md;
mod inject;
mod migrate;
mod session;
mod store;

pub use inject::{inject, memory_context};
pub use session::{Session, Update};
pub use store::{list_projects, project_core, Store};

static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub max_scopes_in_prompt: u32,
    pub max_chars_per_scope: u32,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_scopes_in_prompt: 3,
            max_chars_per_scope: 6000,
        }
    }
}
impl MemoryConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=10).contains(&self.max_scopes_in_prompt),
            "Sessions in core must be 1 to 10"
        );
        ensure!(
            (1000..=20000).contains(&self.max_chars_per_scope),
            "Chars per session must be 1000 to 20000"
        );
        Ok(())
    }
}

fn stamp() -> String {
    format_stamp(now_secs())
}

fn format_stamp(secs: u64) -> String {
    let Civil {
        year,
        month,
        day,
        hour,
        minute,
        second,
    } = civil(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}
pub fn strip_blocks(text: &str) -> (String, Option<String>) {
    let mut clean = String::new();
    let mut rest = text;
    let mut last = None;
    while let Some(start) = rest.find("<memory>") {
        clean.push_str(&rest[..start]);
        rest = &rest[start + 8..];
        let Some(end) = rest.find("</memory>") else {
            return (clean, last);
        };
        last = Some(rest[..end].to_owned());
        rest = &rest[end + 9..];
    }
    clean.push_str(rest);
    (clean, last)
}
pub const INSTRUCTION: &str = concat!(
    "
End the reply with ONE <memory>{\"title\":\"…\",\"status\":\"active\",\"goal\":\"…\",",
    "\"decided\":[\"…\"],\"open\":[\"…\"],\"done\":[\"…\"],\"evidence\":[]}</memory> for THIS chat",
    " (it updates this chat's entry), or <memory>none</memory> if nothing changed. Terse English.",
    " open = all tasks still open; done = newly finished. status: active|blocked|done|obsolete.",
    " done only with evidence (commit hash, or user-confirmed when the user says it is finished).",
    " obsolete when the user drops it. Never invent evidence."
);

#[cfg(test)]
mod tests;
