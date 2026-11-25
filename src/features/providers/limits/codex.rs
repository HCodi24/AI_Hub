use crate::providers::cli_support;
use super::LimitWindow;
use crate::config::ProviderConfig;
use serde_json::{json, Value};

pub(super) fn codex_windows(result: &Value) -> Vec<LimitWindow> {
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

// This helper starts a separate, read-only App Server; it never creates a turn.
pub(super) async fn read_codex(provider: &ProviderConfig) -> Option<Vec<LimitWindow>> {
    let config = crate::config::config_file_path().ok()?;
    let home = crate::providers::codex::home(&config, provider).ok()?;
    if !home.join("auth.json").is_file() { return Some(Vec::new()); }
    let path = cli_support::resolve_executable("codex", Some(&provider.executable))?;
    let mut client = crate::providers::codex::transport::Client::start_without_plugins(&path, &home, &home).await.ok()?;
    let result = client.call("account/rateLimits/read",json!({})).await.ok();
    client.close().await;
    result.map(|result| codex_windows(&result))
}
