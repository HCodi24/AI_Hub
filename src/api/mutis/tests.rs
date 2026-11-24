use super::diagnose::{diagnosis_prompt, failure_events};
use super::roles::pick_lead;
use super::settings::{sync_lead, validate};
use super::stats::summarize;
use crate::{
    config::{self, Config, ProviderConfig},
    mutis::Event,
};
use serde_json::json;
use std::collections::HashMap;

fn source(id: &str, models: &[&str]) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        kind: "codex_cli".into(),
        models: models.iter().map(|m| m.to_string()).collect(),
        ..Default::default()
    }
}

fn role(provider: &str, model: &str, effort: &str) -> config::MutisRole {
    config::MutisRole {
        provider_id: provider.into(),
        model: model.into(),
        effort: effort.into(),
    }
}

#[test]
fn the_lead_keeps_its_role_model_only_while_its_own_source_is_in_use() {
    let mut cfg = config::MutisConfig::default();
    cfg.roles.lead = role("a", "role-model", "max");
    cfg.high_roles.lead = role("b", "high-model", "high");
    let (a, b) = (source("a", &["a-first"]), source("b", &["b-first"]));
    // Whatever the effort, the High roles apply.
    let lead = pick_lead(&cfg, "max", Some(&b), b.clone());
    assert_eq!(
        (lead.model.as_str(), lead.effort.as_str()),
        ("high-model", "high")
    );
    let lead = pick_lead(&cfg, "medium", Some(&b), b.clone());
    assert_eq!(
        (lead.model.as_str(), lead.effort.as_str()),
        ("high-model", "high")
    );
    // The configured source is down: the fallback source starts on its own first model.
    let lead = pick_lead(&cfg, "medium", Some(&b), a.clone());
    assert_eq!(lead.model, "a-first");
}

#[test]
fn a_high_level_without_its_own_lead_follows_the_max_roles() {
    let mut cfg = config::MutisConfig::default();
    cfg.roles.lead = role("a", "role-model", "max");
    let a = source("a", &["a-first"]);
    let lead = pick_lead(&cfg, "medium", Some(&a), a.clone());
    assert_eq!(
        (lead.model.as_str(), lead.effort.as_str()),
        ("role-model", "max")
    );
}

#[test]
fn summarizes_task_counts_success_and_errors() {
    let event = |kind: &str, ok, detail: Option<&str>| Event {
        ts: 1,
        plan_id: None,
        task_id: None,
        provider_id: "codex".into(),
        kind: kind.into(),
        task_kind: None,
        ok,
        duration_ms: None,
        tokens: None,
        detail: detail.map(str::to_owned),
    };
    let stats = summarize(HashMap::from([(
        "codex".into(),
        vec![
            event("task_start", true, None),
            event("task_end", true, None),
            event("task_start", true, None),
            event("task_end", false, Some("timeout")),
        ],
    )]));
    assert_eq!(stats["codex"]["tasks_run"], json!(2));
    assert_eq!(stats["codex"]["success_rate"], json!(0.5));
    assert_eq!(
        stats["codex"]["top_errors"],
        json!([{"message": "timeout", "count": 1}]),
    );
}

#[test]
fn defaults_pass_validation() {
    assert!(validate(&config::MutisConfig::default(), &[]).is_ok());
}

#[test]
fn rejects_retry_attempts_outside_the_limit() {
    let value = config::MutisConfig {
        retry_attempts: 6,
        ..Default::default()
    };
    assert!(validate(&value, &[]).is_err());
}

#[test]
fn rejects_an_unknown_provider_id() {
    let mut value = config::MutisConfig::default();
    value.roles.lead = role("missing", "model", "default");
    assert!(validate(&value, &["known".into()]).is_err());
}

#[test]
fn rejects_an_invalid_effort() {
    let mut value = config::MutisConfig::default();
    value.high_roles.lead = role("known", "model", "invalid");
    assert!(validate(&value, &["known".into()]).is_err());
}

#[test]
fn syncs_the_high_lead_or_falls_back_to_max() {
    let mut config = Config::seeded();
    config.mutis.roles.lead = role("max-source", "max-model", "max");
    config.mutis.high_roles.lead = role("high-source", "high-model", "high");
    for (provider, model) in [("high-source", "high-model"), ("max-source", "max-model")] {
        sync_lead(&mut config);
        assert_eq!(config.planner_provider_id, provider);
        assert_eq!(config.planner_model, model);
        assert_eq!(config.mutis.reviewer_provider_id, provider);
        assert_eq!(config.mutis.reviewer_model, model);
        config.mutis.high_roles.lead.provider_id = " ".into();
    }
}

#[test]
fn failure_events_filters_caps_and_truncates_details() {
    let event = |kind: &str, ok, detail| Event {
        ts: 1,
        plan_id: None,
        task_id: None,
        provider_id: "codex".into(),
        kind: kind.into(),
        task_kind: None,
        ok,
        duration_ms: None,
        tokens: None,
        detail,
    };
    let detail = "ữ".repeat(301);
    let mut events = vec![
        event("verify", true, None),
        event("review", true, None),
        event("task_end", false, None),
        event("verify", false, Some(detail)),
        event("review", false, None),
        event("task_end", true, Some("Empty DeepSeek".into())),
    ];
    for kind in ["retry", "watchdog_kill", "resume", "fix_task", "error"] {
        events.push(event(kind, true, None));
    }
    events.extend((0..150).map(|_| event("retry", true, None)));
    let failures = failure_events(&events);
    assert_eq!(failures.len(), 150);
    assert_eq!(
        failures[..8].iter().map(|event| event.kind.as_str()).collect::<Vec<_>>(),
        ["verify", "review", "task_end", "retry", "watchdog_kill", "resume", "fix_task", "error"]
    );
    assert_eq!(failures[0].detail.as_deref(), Some("ữ".repeat(300).as_str()));
    assert!(failures[3..].iter().all(|event| event.detail.is_none()));
}

#[test]
fn diagnosis_prompt_includes_context_and_reviewer_role() {
    let settings = json!({"retry_attempts": 3});
    let stats = json!({"days": 30});
    let prompt = diagnosis_prompt(&settings, &stats, &[]);
    assert!(prompt.contains("MutiS reviewer"));
    assert!(prompt.contains(&format!("SETTINGS (no provider keys):\n{settings}")));
    assert!(prompt.contains(&format!("STATS:\n{stats}")));
}
