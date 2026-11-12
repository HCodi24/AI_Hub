//! Per-project, per-chat notes; no provider calls live here.
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
    // Gregorian civil date from Unix days, independent of local timezone.
    let secs = crate::mutis::now();
    let z = (secs / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs % 86400 / 3600,
        secs % 3600 / 60,
        secs % 60
    )
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
