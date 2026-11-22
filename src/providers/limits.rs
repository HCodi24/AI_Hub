use super::cli_support;
use crate::config::ProviderConfig;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

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

fn cache() -> &'static Mutex<HashMap<String, Limits>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Limits>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(load_cache()))
}

fn cache_path() -> Option<PathBuf> {
    Some(
        crate::config::config_file_path()
            .ok()?
            .parent()?
            .join("limits-cache.json"),
    )
}

fn load_cache() -> HashMap<String, Limits> {
    cache_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

fn persist_cache(entries: &HashMap<String, Limits>) {
    let Some(path) = cache_path() else {
        return;
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(contents) = serde_json::to_vec_pretty(entries) else {
        return;
    };
    let temporary = path.with_extension(format!("json.{}.tmp", std::process::id()));
    if fs::write(&temporary, &contents).is_err() {
        let _ = fs::remove_file(&temporary);
        return;
    }
    if fs::rename(&temporary, &path).is_err() {
        // Some platforms do not replace an existing destination during rename.
        let _ = fs::write(&path, contents);
        let _ = fs::remove_file(temporary);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn save(id: &str, windows: Vec<LimitWindow>) -> Limits {
    let limits = Limits {
        windows,
        updated_at: Some(now()),
    };
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_owned(), limits.clone());
    limits
}

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
            retain_known_usage(&mut window, cached.as_ref(), now());
            save(id, vec![window]);
            let entries = cache().lock().unwrap_or_else(|e| e.into_inner());
            persist_cache(&entries);
        }
    }
}

fn retain_known_usage(window: &mut LimitWindow, cached: Option<&Limits>, now: u64) {
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

fn claude_window(info: &Value) -> Option<LimitWindow> {
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

fn codex_windows(result: &Value) -> Vec<LimitWindow> {
    let mut windows = Vec::new();
    let buckets = result["rateLimitsByLimitId"].as_object();
    let single = &result["rateLimits"];
    let snapshots: Vec<(&str, &Value)> = match buckets.filter(|b| !b.is_empty()) {
        Some(buckets) => buckets.iter().map(|(k, v)| (k.as_str(), v)).collect(),
        None => vec![("Codex", single)],
    };
    for (name, snapshot) in snapshots {
        for (key, fallback) in [("primary", "Primary"), ("secondary", "Secondary")] {
            let window = &snapshot[key];
            let Some(used) = window["usedPercent"]
                .as_f64()
                .filter(|v| (0.0..=100.0).contains(v))
            else {
                continue;
            };
            let duration = window["windowDurationMins"]
                .as_u64()
                .map(|mins| {
                    if mins % 1440 == 0 {
                        format!("{}d", mins / 1440)
                    } else if mins % 60 == 0 {
                        format!("{}h", mins / 60)
                    } else {
                        format!("{mins}m")
                    }
                })
                .unwrap_or_else(|| fallback.to_owned());
            windows.push(LimitWindow {
                label: format!(
                    "{} · {duration}",
                    snapshot["limitName"].as_str().unwrap_or(name)
                ),
                used_percent: Some(used),
                status: if used >= 100.0 {
                    "rejected"
                } else if used >= 80.0 {
                    "allowed_warning"
                } else {
                    "allowed"
                }
                .to_owned(),
                resets_at: window["resetsAt"].as_u64(),
            });
        }
    }
    windows
}

/// Claude Code's own login, if it is usable now (expired tokens are left to the CLI to refresh).
fn claude_token(provider: &ProviderConfig) -> Option<String> {
    let config = crate::config::config_file_path().ok()?;
    let home = super::claude::environment::home(&config, provider).ok()?;
    let value: Value = serde_json::from_slice(&fs::read(home.join(".credentials.json")).ok()?).ok()?;
    let oauth = &value["claudeAiOauth"];
    if oauth["expiresAt"].as_u64().is_some_and(|ms| ms / 1000 <= now()) { return None; }
    oauth["accessToken"].as_str().filter(|token| !token.is_empty()).map(str::to_owned)
}

/// `2025-11-04T04:59:59.943648+00:00` -> Unix seconds (fraction ignored, offset applied).
fn iso_seconds(text: &str) -> Option<u64> {
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
fn usage_windows(body: &Value) -> Vec<LimitWindow> {
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

fn usage_attempts() -> &'static Mutex<HashMap<String, u64>> {
    static ATTEMPTS: OnceLock<Mutex<HashMap<String, u64>>> = OnceLock::new();
    ATTEMPTS.get_or_init(Default::default)
}

/// shortcut: an undocumented endpoint; any failure (expired login, 429, a changed answer) keeps what the CLI last reported.
async fn read_claude_usage(provider: &ProviderConfig) -> Option<Vec<LimitWindow>> {
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
            let due = now().saturating_sub(last) >= 60;
            if due { attempts.insert(provider.id.clone(), now()); }
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
    if provider.kind != "codex_cli"
        || cached
            .updated_at
            .is_some_and(|time| now().saturating_sub(time) < 30)
    {
        return cached;
    }
    match read_codex(provider).await {
        Some(windows) => save(&provider.id, windows),
        None => cached,
    }
}

// This helper starts a separate, read-only App Server; it never creates a turn.
async fn read_codex(provider: &ProviderConfig) -> Option<Vec<LimitWindow>> {
    let config = crate::config::config_file_path().ok()?;
    let home = super::codex::home(&config, provider).ok()?;
    if !home.join("auth.json").is_file() { return Some(Vec::new()); }
    let path = cli_support::resolve_executable("codex", Some(&provider.executable))?;
    let mut client = super::codex::transport::Client::start(&path, &home, &home).await.ok()?;
    let result = client.call("account/rateLimits/read",json!({})).await.ok();
    client.close().await;
    result.map(|result| codex_windows(&result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_usage_endpoint_answer_becomes_windows() {
        assert_eq!(iso_seconds("1970-01-02T00:00:00+00:00"), Some(86_400));
        assert_eq!(iso_seconds("2025-11-04T04:59:59.943648+00:00"), Some(1_762_232_399));
        assert_eq!(iso_seconds("2025-11-04T05:59:59Z"), Some(1_762_235_999));
        assert_eq!(iso_seconds("2025-11-04T06:59:59+02:00"), Some(1_762_232_399));
        assert_eq!(iso_seconds("not a date"), None);
        let windows = usage_windows(&json!({
            "five_hour": {"utilization": 37.0, "resets_at": "2025-11-04T04:59:59.943648+00:00"},
            "seven_day": {"utilization": 85.5, "resets_at": null},
            "seven_day_opus": null, "extra_usage": {"is_enabled": false}
        }));
        assert_eq!(windows.len(), 2);
        assert_eq!((windows[0].label.as_str(), windows[0].status.as_str(), windows[0].resets_at), ("five_hour", "allowed", Some(1_762_232_399)));
        assert_eq!((windows[1].label.as_str(), windows[1].status.as_str(), windows[1].resets_at), ("seven_day", "allowed_warning", None));
        assert!(usage_windows(&json!({"error": "x"})).is_empty());
    }

    #[test]
    fn claude_keeps_unknown_usage_and_rejected_status() {
        let unknown = claude_window(&json!({"status":"allowed"})).unwrap();
        assert_eq!(unknown.used_percent, None);
        let rejected =
            claude_window(&json!({"status":"rejected","utilization":1.0,"resetsAt":123})).unwrap();
        assert_eq!(rejected.used_percent, Some(100.0));
        assert_eq!(rejected.resets_at, Some(123));
        assert!(claude_window(&json!({"status":"other"})).is_none());
        assert_eq!(
            claude_window(&json!({"status":"allowed","utilization":2}))
                .unwrap()
                .used_percent,
            None
        );
    }

    #[test]
    fn claude_keeps_known_usage_until_the_matching_window_resets() {
        let mut window = claude_window(&json!({
            "status":"allowed",
            "rateLimitType":"five_hour"
        }))
        .unwrap();
        let cached = Limits {
            windows: vec![LimitWindow {
                label: "five_hour".to_owned(),
                used_percent: Some(42.0),
                status: "allowed_warning".to_owned(),
                resets_at: Some(200),
            }],
            updated_at: Some(100),
        };

        retain_known_usage(&mut window, Some(&cached), 199);
        assert_eq!(window.used_percent, Some(42.0));

        let mut expired = claude_window(&json!({
            "status":"allowed",
            "rateLimitType":"five_hour"
        }))
        .unwrap();
        retain_known_usage(&mut expired, Some(&cached), 200);
        assert_eq!(expired.used_percent, None);
    }

    #[test]
    fn codex_uses_all_buckets_without_duplicate_fallback() {
        let result = json!({"rateLimits":{"primary":{"usedPercent":99}},"rateLimitsByLimitId":{
            "codex":{"primary":{"usedPercent":25,"windowDurationMins":300,"resetsAt":123},"secondary":{"usedPercent":100,"windowDurationMins":10080}},
            "other":{"primary":{"usedPercent":85}},"invalid":{"primary":{"usedPercent":150}}
        }});
        let windows = codex_windows(&result);
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[0].label, "codex · 5h");
        assert_eq!(windows[1].status, "rejected");
        assert_eq!(windows[2].status, "allowed_warning");
        assert!(codex_windows(&json!({"rateLimits":null})).is_empty());
        assert_eq!(
            codex_windows(&json!({"rateLimits":{"primary":{"usedPercent":0}}}))[0].used_percent,
            Some(0.0)
        );
    }
}
