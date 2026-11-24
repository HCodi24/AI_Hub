use crate::config::ProviderConfig;
use crate::core::time::now_secs;
use serde::{Deserialize, Serialize};

mod antigravity;
mod cache;
mod claude;
mod codex;
#[cfg(test)]
mod tests;

use antigravity::read_antigravity;
use cache::{cache, persist_cache, save};
use claude::{read_claude_usage, usage_attempts};
use codex::read_codex;
pub(crate) use claude::record_claude;

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

pub(crate) async fn read(provider: &ProviderConfig, ask_anthropic: bool) -> Limits {
    let cached = cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get(&provider.id)
        .cloned()
        .unwrap_or_default();
    if provider.kind == "claude_cli" && ask_anthropic {
        // At most once a minute per source, whether or not the last try worked.
        let due = {
            let mut attempts = usage_attempts().lock().unwrap_or_else(|e| e.into_inner());
            let last = attempts.get(&provider.id).copied().unwrap_or(0);
            let due = now_secs().saturating_sub(last) >= 60;
            if due { attempts.insert(provider.id.clone(), now_secs()); }
            due
        };
        if due {
            if let Some(windows) = read_claude_usage(provider).await {
                let limits = save(&provider.id, windows);
                persist_cache(&cache().lock().unwrap_or_else(|e| e.into_inner()));
                return limits;
            }
        }
        return cached;
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
