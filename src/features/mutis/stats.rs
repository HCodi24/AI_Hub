use super::{normalize_error, Event, EventLog};
use serde_json::{json, Value};
use std::collections::HashMap;

pub fn build_stats(event_log: &EventLog, days: u64) -> anyhow::Result<Value> {
    let days = days.clamp(1, 365);
    let cutoff = crate::core::time::now_secs().saturating_sub(days.saturating_mul(86400));
    let events = event_log.recent(2000)?;
    let events = events
        .into_iter()
        .filter(|event| event.ts >= cutoff)
        .collect::<Vec<_>>();
    let mut providers: HashMap<String, Vec<Event>> = HashMap::new();
    let mut kinds: HashMap<String, Vec<Event>> = HashMap::new();
    for event in &events {
        providers
            .entry(event.provider_id.clone())
            .or_default()
            .push(event.clone());
        kinds
            .entry(event.task_kind.clone().unwrap_or_else(|| "không rõ".into()))
            .or_default()
            .push(event.clone());
    }
    Ok(json!({
        "days": days,
        "providers": summarize(providers),
        "task_kinds": summarize(kinds),
        "events": events.into_iter().rev().take(50).collect::<Vec<_>>()
    }))
}

pub(super) fn summarize(groups: HashMap<String, Vec<Event>>) -> Value {
    Value::Object(
        groups
            .into_iter()
            .map(|(key, group)| {
                let starts = group.iter().filter(|e| e.kind == "task_start").count();
                let ends = group
                    .iter()
                    .filter(|e| e.kind == "task_end")
                    .collect::<Vec<_>>();
                let ok = ends.iter().filter(|e| e.ok).count();
                let duration = ends.iter().filter_map(|e| e.duration_ms).sum::<u64>();
                let tokens = ends.iter().filter_map(|e| e.tokens).sum::<u64>();
                let mut errors: HashMap<String, usize> = HashMap::new();
                for event in group.iter().filter(|e| !e.ok || e.kind == "error") {
                    if let Some(detail) = &event.detail {
                        *errors.entry(normalize_error(detail)).or_default() += 1;
                    }
                }
                let mut errors = errors.into_iter().collect::<Vec<_>>();
                errors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
                let top_errors = errors
                    .into_iter()
                    .take(5)
                    .map(|(message, count)| json!({"message":message,"count":count}))
                    .collect::<Vec<_>>();
                (
                    key,
                    json!({
                        "tasks_run": starts,
                        "success_rate": if ends.is_empty() {
                            0.0
                        } else {
                            ok as f64 / ends.len() as f64
                        },
                        "avg_duration_ms": if ends.is_empty() {
                            0
                        } else {
                            duration / ends.len() as u64
                        },
                        "total_tokens": tokens,
                        "retries": group.iter().filter(|e| e.kind == "retry").count(),
                        "watchdog_kills": group
                            .iter()
                            .filter(|e| e.kind == "watchdog_kill")
                            .count(),
                        "resumes": group.iter().filter(|e| e.kind == "resume").count(),
                        "verify_fail_rate": rate(&group, "verify", false),
                        "review_fail_rate": rate(&group, "review", false),
                        "fix_rounds": group.iter().filter(|e| e.kind == "fix_task").count(),
                        "top_errors": top_errors
                    }),
                )
            })
            .collect(),
    )
}

fn rate(events: &[Event], kind: &str, ok: bool) -> f64 {
    let matching = events.iter().filter(|e| e.kind == kind).collect::<Vec<_>>();
    if matching.is_empty() {
        0.0
    } else {
        matching.iter().filter(|e| e.ok == ok).count() as f64 / matching.len() as f64
    }
}
