use super::antigravity::antigravity_windows;
use super::claude::{claude_window, iso_seconds, retain_known_usage, usage_windows};
use super::codex::codex_windows;
use super::{LimitWindow, Limits};
use serde_json::json;

#[test]
fn antigravity_usage_lines_become_windows() {
    let windows = antigravity_windows("Gemini Models	Weekly Limit Remaining	86%	2026-11-28T08:49:28Z
not a row
Claude and GPT models	Five Hour Limit Remaining	0%	2026-11-23T11:49:58Z
");
    assert_eq!(windows.len(), 2);
    assert_eq!(windows[0].label, "Gemini · Weekly");
    assert_eq!(windows[0].used_percent, Some(14.0));
    assert!(windows[0].resets_at.is_some());
    assert_eq!(windows[1].status, "rejected");
}

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
