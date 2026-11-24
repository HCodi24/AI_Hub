use super::{
    apply_setting_change,
    diagnosis::{diagnosis_prompt, failure_events, validate_diagnosis},
    normalize_error, parse_diagnosis,
    roles::{resolve_lead, resolve_roles},
    setting::{sync_lead, validate},
    stats::summarize,
    Event,
};
use crate::{
    api::providers::ConnectionTest,
    app::AppState,
    config::{self, Config, ProviderConfig},
};
use serde_json::{json, Value};
use std::collections::HashMap;

#[test]
fn parses_diagnosis_json_and_normalizes_numbered_errors() {
    let parsed = parse_diagnosis(
        "```json\n{\"health\":\"tốt\",\"findings\":[],\"summary\":\"Ổn\",\"extra\":true}\n```",
    )
    .unwrap();
    assert_eq!(parsed["health"], "tốt");
    assert_eq!(
        normalize_error("HTTP 503 on attempt 12"),
        "HTTP # on attempt #"
    );
}

#[test]
fn validates_finding_shape_and_drops_unknown_keys() {
    let valid = json!({"health":"kém","summary":"Sự cố","extra":1,"findings":[{"problem":"p","evidence":"e","cause":"c","severity":"high","suggestion":"s","setting_change":null,"extra":true}]});
    let parsed = validate_diagnosis(valid).unwrap();
    assert!(parsed.get("extra").is_none());
    assert!(parsed["findings"][0].get("extra").is_none());
    assert!(
        validate_diagnosis(json!({"health":"unknown","summary":"x","findings":[]})).is_err()
    );
    assert!(validate_diagnosis(json!({"health":"tốt","summary":3,"findings":[]})).is_err());
}

#[test]
fn setting_change_allowlist_accepts_and_rejects_required_paths() {
    let mut config = crate::config::Config::seeded();
    let (old, new) =
        apply_setting_change(&mut config, "mutis.watchdog_minutes", &json!(8)).unwrap();
    assert_eq!(old, json!(5));
    assert_eq!(new, json!(8));
    config.providers.push(source("known", &["test-model"]));
    for role in ["lead", "worker", "checker"] {
        for (field, value) in [
            ("provider_id", json!("known")),
            ("model", json!("test-model")),
            ("effort", json!("high")),
        ] {
            let path = format!("mutis.{role}.{field}");
            let (_, new) = apply_setting_change(&mut config, &path, &value).unwrap();
            assert_eq!(new, value, "{path}");
        }
    }
    for path in [
        "mutis.roles.lead.provider_id",
        "mutis.roles.lead.api_key",
        concat!("mutis.high_", "roles.lead.model"),
        "mutis.routing[2].role",
        concat!("mutis.deepseek_", "ui"),
        "mutis.lead.permission",
        "mutis.checker.count",
        "providers[0].api_key",
        "mutis.unknown",
        "mutis.verify_command",
        "mutis.worker_rules",
        "mutis.paths",
    ] {
        assert_eq!(
            apply_setting_change(&mut config, path, &Value::Null).unwrap_err(),
            "Không được phép đổi thiết lập này",
            "{path}"
        );
    }
    assert!(apply_setting_change(&mut config, "mutis.watchdog_minutes", &json!(61)).is_err());
    assert!(apply_setting_change(&mut config, "mutis.retry_attempts", &json!(0)).is_err());
    assert_eq!(
        apply_setting_change(&mut config, "mutis.worker.provider_id", &json!("missing"))
            .unwrap_err(),
        "Nguồn AI không tồn tại"
    );
    apply_setting_change(&mut config, "mutis.worker.provider_id", &json!("")).unwrap();
    assert!(config.mutis.worker.provider_id.is_empty());
}

#[test]
fn worker_and_checker_permissions_are_checked() {
    let mut config = Config::seeded();
    for (path, permission) in [
        ("mutis.worker.permission", "read"),
        ("mutis.checker.permission", "write"),
    ] {
        let (old, new) = apply_setting_change(&mut config, path, &json!(permission)).unwrap();
        assert_eq!((old, new), (json!("full"), json!(permission)));
        assert_eq!(
            apply_setting_change(&mut config, path, &json!("admin")).unwrap_err(),
            "Quyền không hợp lệ"
        );
    }
    assert_eq!(config.mutis.worker.permission, "read");
    assert_eq!(config.mutis.checker.permission, "write");
    let mut value = config::MutisConfig::default();
    value.worker.permission = "admin".into();
    assert_eq!(validate(&value, &[]), Err("Quyền không hợp lệ"));
    value.worker.permission = "full".into();
    value.checker.permission = "admin".into();
    assert_eq!(validate(&value, &[]), Err("Quyền không hợp lệ"));
}

#[test]
fn worker_count_is_bounded_for_partial_and_full_settings() {
    let mut config = Config::seeded();
    for count in [1, 3] {
        apply_setting_change(&mut config, "mutis.worker.count", &json!(count)).unwrap();
        assert_eq!(config.mutis.worker.count, count);
        assert!(validate(&config.mutis, &[]).is_ok());
    }
    for count in [0, 4] {
        assert_eq!(
            apply_setting_change(&mut config, "mutis.worker.count", &json!(count)).unwrap_err(),
            "Giá trị ngoài giới hạn"
        );
        assert_eq!(config.mutis.worker.count, 3);
        let mut value = config::MutisConfig::default();
        value.worker.count = count;
        assert_eq!(validate(&value, &[]), Err("Giá trị ngoài giới hạn"));
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
    value.lead = role("missing", "model", "default");
    assert_eq!(validate(&value, &["known".into()]), Err("Nguồn AI không tồn tại"));
}

#[test]
fn rejects_an_invalid_effort() {
    let mut value = config::MutisConfig::default();
    value.checker.effort = "invalid".into();
    assert_eq!(validate(&value, &[]), Err("Mức suy luận không hợp lệ"));
    value.checker.effort.clear();
    assert!(validate(&value, &[]).is_ok());
    value.checker.model = "configured-model".into();
    assert_eq!(validate(&value, &[]), Err("Mức suy luận không hợp lệ"));
}

#[test]
fn syncs_planner_and_reviewer_from_the_lead() {
    let mut config = Config::seeded();
    config.planner_provider_id = "stale-planner".into();
    config.mutis.reviewer_provider_id = "stale-reviewer".into();
    assert!(validate(&config.mutis, &[]).is_ok());
    sync_lead(&mut config);
    assert!(config.planner_provider_id.is_empty());
    assert!(config.mutis.reviewer_provider_id.is_empty());

    config.providers.push(source("new-lead", &["new-model"]));
    apply_setting_change(&mut config, "mutis.lead.provider_id", &json!("new-lead")).unwrap();
    assert_eq!(config.planner_provider_id, "new-lead");
    assert_eq!(config.mutis.reviewer_provider_id, "new-lead");
    apply_setting_change(&mut config, "mutis.lead.model", &json!("new-model")).unwrap();
    assert_eq!(config.planner_model, "new-model");
    assert_eq!(config.mutis.reviewer_model, "new-model");
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

fn source(id: &str, models: &[&str]) -> ProviderConfig {
    ProviderConfig {
        id: id.into(),
        kind: "codex_cli".into(),
        models: models.iter().map(|m| m.to_string()).collect(),
        ..Default::default()
    }
}

#[tokio::test]
async fn resolves_only_the_configured_connected_lead_and_roles() {
    let mut config = Config::seeded();
    let mut lead_source = source("test-lead", &["first-model"]);
    lead_source.kind = "openai_compat".into();
    config.providers.push(lead_source);
    config.providers.push(source("test-worker", &["worker-model"]));
    config.mutis.worker.provider_id = "test-worker".into();

    let root = std::env::temp_dir().join(format!("mutis-roles-{}", uuid::Uuid::new_v4()));
    let state = AppState::new(config, root.join("config.json"), root.join("chats"), None, 0);
    cache_connection(&state, "test-lead", "disconnected").await;
    cache_connection(&state, "test-worker", "connected").await;

    assert!(resolve_lead(&state).await.is_none());
    state.config.write().await.mutis.lead.provider_id = "unknown-source".into();
    assert!(resolve_lead(&state).await.is_none());
    state.config.write().await.mutis.lead = role("test-lead", "selected-model", "high");
    assert!(resolve_lead(&state).await.is_none());

    let [(lead_name, lead), (worker_name, worker), (checker_name, checker)] =
        resolve_roles(&state).await;
    assert_eq!([lead_name, worker_name, checker_name], ["lead", "worker", "checker"]);
    assert_eq!(lead.configured.unwrap().id, "test-lead");
    assert_eq!(lead.status.unwrap().status, "disconnected");
    assert!(lead.used.is_none());
    assert_eq!(worker.used.unwrap().id, "test-worker");
    assert!(checker.configured.is_none() && checker.status.is_none() && checker.used.is_none());

    cache_connection(&state, "test-lead", "connected").await;
    let lead = resolve_lead(&state).await.unwrap();
    assert_eq!(lead.provider.id, "test-lead");
    assert_eq!(lead.model, "selected-model");
    assert_eq!(lead.effort, "high");
    state.config.write().await.mutis.lead.model.clear();
    assert_eq!(resolve_lead(&state).await.unwrap().model, "first-model");
    std::fs::remove_dir_all(root).unwrap();
}

async fn cache_connection(state: &AppState, provider_id: &str, status: &'static str) {
    state.provider_status.lock().await.insert(
        provider_id.to_owned(),
        (
            std::time::Instant::now(),
            ConnectionTest {
                ok: status == "connected",
                status,
                message: String::new(),
                detail: String::new(),
                plan: None,
                email: None,
            },
        ),
    );
}
