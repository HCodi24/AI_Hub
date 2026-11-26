use crate::config::ProviderConfig;
use crate::core::time::now_secs;
use serde::{Deserialize, Serialize};

mod antigravity;
mod cache;
mod claude;
mod codex;
pub mod quota;
#[cfg(test)]
mod tests;

use antigravity::read_antigravity;
use cache::{cache, persist_cache, save};
use claude::{merge_windows, read_claude_usage, usage_attempts};
use codex::read_codex;
pub(crate) use claude::{claude_limit_warning, record_claude};

pub(crate) fn invalidate(id: &str) {
    let mut entries = cache().lock().unwrap_or_else(|e| e.into_inner());
    entries.remove(id);
    persist_cache(&entries);
}

#[derive(Clone, Default, Deserialize, Serialize)]
pub(crate) struct Limits {
    pub windows: Vec<LimitWindow>,
    pub updated_at: Option<u64>,
}

#[derive(Clone, Deserialize, Serialize)]
pub(crate) struct LimitWindow {
    label: String,
    used_percent: Option<f64>,
    status: String,
    resets_at: Option<u64>,
}

fn with_claude_defaults(mut limits: Limits) -> Limits {
    for label in ["five_hour", "seven_day"] {
        if !limits.windows.iter().any(|window| window.label == label) {
            limits.windows.push(LimitWindow {
                label: label.to_owned(),
                used_percent: None,
                status: "allowed".to_owned(),
                resets_at: None,
            });
        }
    }
    limits.windows.sort_by(|a, b| a.label.cmp(&b.label));
    limits
}

/// Why a source stays frozen. A frozen source whose fresh limits show room again (its window reset early, or the
/// error was a short rate limit) is thawed at once instead of waiting out the fixed freeze.
pub(crate) async fn frozen_now(provider: &ProviderConfig) -> Option<String> {
    let why = quota::frozen(&provider.id)?;
    let fresh = read(provider, true).await;
    if has_room(&fresh.windows) {
        quota::thaw(&provider.id);
        return None;
    }
    Some(why)
}

/// Known windows, none spent. No windows (an API source, or a read that failed) proves nothing.
fn has_room(windows: &[LimitWindow]) -> bool {
    windows.iter().any(|w| w.used_percent.is_some())
        && windows.iter().all(|w| w.status != "rejected" && w.used_percent.map_or(true, |used| used < 100.0))
}

pub(crate) async fn read(provider: &ProviderConfig, ask_anthropic: bool) -> Limits {
    let cached = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    if provider.kind == "claude_cli" {
        const POLL_INTERVAL_SECS: u64 = 60;
        const FAILURE_BACKOFF_SECS: u64 = 300;
        let due = ask_anthropic && {
            let mut attempts = usage_attempts().lock().unwrap_or_else(|e| e.into_inner());
            let now = now_secs();
            let last = attempts.get(&provider.id).copied().unwrap_or(0);
            let due = now.saturating_sub(last) >= POLL_INTERVAL_SECS;
            if due {
                attempts.insert(provider.id.clone(), now);
            }
            due
        };
        if due {
            if let Some(windows) = read_claude_usage(provider).await {
                let windows = merge_windows(
                    cache()
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&provider.id),
                    windows,
                );
                let limits = save(&provider.id, windows);
                persist_cache(&cache().lock().unwrap_or_else(|e| e.into_inner()));
                return with_claude_defaults(limits);
            }
            // Shift the last attempt so the normal interval expires after the failure backoff.
            usage_attempts()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(
                    provider.id.clone(),
                    now_secs().saturating_add(FAILURE_BACKOFF_SECS - POLL_INTERVAL_SECS),
                );
        }
        return with_claude_defaults(cached);
    }
    if provider.kind == "antigravity_cli" {
        if cached.updated_at.is_some_and(|time| now_secs().saturating_sub(time) < 60) {
            return cached;
        }
        return match read_antigravity(provider).await {
            Some(windows) => save(&provider.id, windows),
            None => cached,
        };
    }
    if provider.kind != "codex_cli"
        || cached
            .updated_at
            .is_some_and(|time| now_secs().saturating_sub(time) < 30)
    {
        return cached;
    }
    match read_codex(provider).await {
        Some(windows) => save(&provider.id, windows),
        None => cached,
    }
}
