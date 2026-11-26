use super::antigravity::antigravity_windows;
use super::claude::{
    claude_window, iso_seconds, merge_window, merge_windows, retain_known_usage, usage_windows,
};
use super::codex::codex_windows;
use super::{with_claude_defaults, LimitWindow, Limits};
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
fn claude_merges_a_rate_limit_event_and_keeps_other_windows_even_after_reset() {
    let window = |label: &str, used_percent, resets_at| LimitWindow {
        label: label.to_owned(),
        used_percent: Some(used_percent),
        status: "allowed".to_owned(),
        resets_at: Some(resets_at),
    };
    let cached = Limits {
        windows: vec![
            window("five_hour", 20.0, 200),
            window("seven_day", 40.0, 500),
            window("expired", 10.0, 50),
        ],
        updated_at: Some(90),
    };

    let windows = merge_window(Some(&cached), window("five_hour", 30.0, 250));
    let usage: Vec<_> = windows
        .iter()
        .map(|entry| (entry.label.as_str(), entry.used_percent, entry.resets_at))
        .collect();
    assert_eq!(
        usage,
        [
            ("expired", Some(10.0), Some(50)),
            ("five_hour", Some(30.0), Some(250)),
            ("seven_day", Some(40.0), Some(500)),
        ]
    );
}

#[test]
fn claude_merges_partial_oauth_usage_with_cached_windows() {
    let cached = Limits {
        windows: usage_windows(&json!({
            "five_hour": {"utilization": 20.0},
            "seven_day": {"utilization": 40.0}
        })),
        updated_at: Some(90),
    };
    let incoming = usage_windows(&json!({"five_hour": {"utilization": 30.0}}));

    let windows = merge_windows(Some(&cached), incoming);
    let usage: Vec<_> = windows
        .iter()
        .map(|entry| (entry.label.as_str(), entry.used_percent))
        .collect();
    assert_eq!(usage, [("five_hour", Some(30.0)), ("seven_day", Some(40.0))]);
}

#[test]
fn claude_adds_placeholders_only_for_missing_default_windows() {
    let empty = with_claude_defaults(Limits::default());
    let labels: Vec<_> = empty
        .windows
        .iter()
        .map(|window| window.label.as_str())
        .collect();
    assert_eq!(labels, ["five_hour", "seven_day"]);
    for window in &empty.windows {
        assert_eq!(window.used_percent, None);
        assert_eq!(window.status, "allowed");
        assert_eq!(window.resets_at, None);
    }

    let limits = with_claude_defaults(Limits {
        windows: vec![LimitWindow {
            label: "five_hour".to_owned(),
            used_percent: Some(82.0),
            status: "allowed_warning".to_owned(),
            resets_at: Some(200),
        }],
        updated_at: Some(100),
    });
    assert_eq!(limits.updated_at, Some(100));
    assert_eq!(limits.windows.len(), 2);
    assert_eq!(limits.windows[0].label, "five_hour");
    assert_eq!(limits.windows[0].used_percent, Some(82.0));
    assert_eq!(limits.windows[0].status, "allowed_warning");
    assert_eq!(limits.windows[0].resets_at, Some(200));
    assert_eq!(limits.windows[1].label, "seven_day");
    assert_eq!(limits.windows[1].used_percent, None);
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

#[test]
fn only_known_windows_with_room_thaw_a_frozen_source() {
    let window = |used: Option<f64>, status: &str| super::LimitWindow { label: "five_hour".into(), used_percent: used, status: status.into(), resets_at: None };
    assert!(super::has_room(&[window(Some(3.0), "allowed")]), "a reset window has room");
    assert!(!super::has_room(&[window(Some(100.0), "allowed")]));
    assert!(!super::has_room(&[window(Some(10.0), "rejected")]));
    assert!(!super::has_room(&[window(None, "allowed")]), "placeholder windows prove nothing");
    assert!(!super::has_room(&[]));
}
