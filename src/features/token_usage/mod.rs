//! Per-chat token history, independent of memory and of the Dashboard.
//!
//! Every accepted token log line of an assistant message is aggregated into cumulative per-model
//! totals (the same "last valid report per message" rule the chat UI uses) and appended as one
//! JSONL snapshot at `<chats>/token-usage/<chat uuid>.jsonl`. Only counts and the model name are
//! stored: never content, prompts or memory data.

mod parse;
mod store;

pub use parse::is_token_report;
pub use store::{history, record};

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

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
    pub(super) fn validate(&self) -> Result<()> {
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
    pub(super) fn canonicalize(&mut self) {
        self.models.sort_by(|left, right| left.model.cmp(&right.model));
    }
}

fn lock() -> std::sync::MutexGuard<'static, ()> {
    STORE_LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

#[cfg(test)]
mod tests;
