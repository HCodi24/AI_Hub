use super::cache::{cache, persist_cache, save};
use super::{LimitWindow, Limits};
use crate::config::ProviderConfig;
use crate::core::time::now_secs;
use serde_json::Value;
use std::{
    collections::HashMap,
    fs,
    sync::{Mutex, OnceLock},
};

pub(crate) fn record_claude(id: &str, line: &str) {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return;
    };
    if value["type"] == "rate_limit_event" {
        if let Some(window) = claude_window(&value["rate_limit_info"]) {
            let mut window = window;
            let cached = cache()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .get(id)
                .cloned();
            retain_known_usage(&mut window, cached.as_ref(), now_secs());
            save(id, vec![window]);
            let entries = cache().lock().unwrap_or_else(|e| e.into_inner());
            persist_cache(&entries);
        }
    }
}

pub(super) fn retain_known_usage(window: &mut LimitWindow, cached: Option<&Limits>, now: u64) {
    if window.used_percent.is_some() {
        return;
    }
    window.used_percent = cached
        .and_then(|limits| {
            limits.windows.iter().find(|previous| {
                previous.label == window.label
                    && previous.used_percent.is_some()
                    && previous.resets_at.is_some_and(|resets_at| now < resets_at)
            })
        })
        .and_then(|previous| previous.used_percent);
}

pub(super) fn claude_window(info: &Value) -> Option<LimitWindow> {
    let status = info["status"].as_str()?;
    if !matches!(status, "allowed" | "allowed_warning" | "rejected") {
        return None;
    }
    Some(LimitWindow {
        label: info["rateLimitType"]
            .as_str()
            .unwrap_or("Claude CLI")
            .to_owned(),
        used_percent: info["utilization"]
            .as_f64()
            .filter(|v| (0.0..=1.0).contains(v))
            .map(|v| v * 100.0),
        status: status.to_owned(),
        resets_at: info["resetsAt"]
            .as_u64()
            .or_else(|| info["resets_at"].as_u64()),
    })
}

/// Claude Code's own login, if it is usable now (expired tokens are left to the CLI to refresh).
fn claude_token(provider: &ProviderConfig) -> Option<String> {
    let config = crate::config::config_file_path().ok()?;
    let home = crate::providers::claude::environment::home(&config, provider).ok()?;
    let value: Value = serde_json::from_slice(&fs::read(home.join(".credentials.json")).ok()?).ok()?;
    let oauth = &value["claudeAiOauth"];
    if oauth["expiresAt"].as_u64().is_some_and(|ms| ms / 1000 <= now_secs()) { return None; }
    oauth["accessToken"].as_str().filter(|token| !token.is_empty()).map(str::to_owned)
}

/// `2025-11-04T04:59:59.943648+00:00` -> Unix seconds (fraction ignored, offset applied).
pub(super) fn iso_seconds(text: &str) -> Option<u64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let offset = match text.get(text.len().saturating_sub(6)..)? {
        tail if tail.starts_with(['+', '-']) && tail.as_bytes()[3] == b':' => {
            let sign = if tail.starts_with('-') { -1 } else { 1 };
            sign * (tail.get(1..3)?.parse::<i64>().ok()? * 3600 + tail.get(4..6)?.parse::<i64>().ok()? * 60)
        }
        _ => 0,
    };
    // Days since 1970-01-01 (proleptic Gregorian).
    let (y, m) = if month <= 2 { (year - 1, month + 9) } else { (year, month - 3) };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * m + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    u64::try_from(days * 86_400 + hour * 3600 + minute * 60 + second - offset).ok()
}

/// The `/api/oauth/usage` answer: one entry per window (`five_hour`, `seven_day`, ...) with `utilization` in percent.
pub(super) fn usage_windows(body: &Value) -> Vec<LimitWindow> {
    let Some(map) = body.as_object() else { return Vec::new() };
    let mut windows: Vec<LimitWindow> = map
        .iter()
        .filter_map(|(label, window)| {
            let used = window["utilization"].as_f64().filter(|v| (0.0..=100.0).contains(v))?;
            Some(LimitWindow {
                label: label.clone(),
                used_percent: Some(used),
                status: if used >= 100.0 { "rejected" } else if used >= 80.0 { "allowed_warning" } else { "allowed" }.to_owned(),
                resets_at: window["resets_at"].as_str().and_then(iso_seconds),
            })
        })
        .collect();
    windows.sort_by(|a, b| a.label.cmp(&b.label));
    windows
}

pub(super) fn usage_attempts() -> &'static Mutex<HashMap<String, u64>> {
    static ATTEMPTS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    ATTEMPTS.get_or_init(Default::default)
}

/// shortcut: an undocumented endpoint; any failure (expired login, 429, a changed answer) keeps what the CLI last reported.
pub(super) async fn read_claude_usage(provider: &ProviderConfig) -> Option<Vec<LimitWindow>> {
    let token = claude_token(provider)?;
    let response = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().ok()?
        .get("https://api.anthropic.com/api/oauth/usage")
        .bearer_auth(token)
        .header("anthropic-beta", "oauth-2025-04-20")
        .header("User-Agent", "ai-hub")
        .send().await.ok()?;
    if !response.status().is_success() { return None; }
    let windows = usage_windows(&response.json::<Value>().await.ok()?);
    (!windows.is_empty()).then_some(windows)
}
