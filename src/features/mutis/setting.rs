use crate::config::{Config, MutisConfig};
use serde_json::{json, Value};

pub fn replace_settings(config: &mut Config, value: &MutisConfig) -> Result<(), &'static str> {
    let provider_ids: Vec<_> = config.providers.iter().map(|p| p.id.clone()).collect();
    validate(value, &provider_ids)?;
    config.mutis = value.clone();
    sync_lead(config);
    Ok(())
}

pub(super) fn validate(value: &MutisConfig, provider_ids: &[String]) -> Result<(), &'static str> {
    if !(1..=5).contains(&value.retry_attempts)
        || !(1..=60).contains(&value.watchdog_minutes)
        || value.resume_max > 3
        || value.fix_rounds_max > 5
        || !(50..=5000).contains(&value.deepseek_max_file_lines)
        || !(5..=40).contains(&value.deepseek_max_steps)
        || !(1..=3).contains(&value.debate_rounds)
        || value.retry_base_ms > 60000
        || !(1..=3).contains(&value.worker.count)
    {
        return Err("Giá trị ngoài giới hạn");
    }
    let MutisConfig { lead, worker, checker, .. } = value;
    for (provider_id, model, effort) in [
        (&lead.provider_id, &lead.model, &lead.effort),
        (&worker.provider_id, &worker.model, &worker.effort),
        (&checker.provider_id, &checker.model, &checker.effort),
    ] {
        if !provider_id.is_empty() && !provider_ids.contains(provider_id) {
            return Err("Nguồn AI không tồn tại");
        }
        if !valid_effort(provider_id, model, effort) {
            return Err("Mức suy luận không hợp lệ");
        }
    }
    if !valid_permission(&worker.permission) || !valid_permission(&checker.permission) {
        return Err("Quyền không hợp lệ");
    }
    Ok(())
}

fn valid_effort(provider_id: &str, model: &str, effort: &str) -> bool {
    // An unconfigured role may have no effort override.
    (provider_id.is_empty() && model.is_empty() && effort.is_empty())
        || matches!(effort, "default" | "low" | "medium" | "high" | "xhigh" | "max")
}

fn valid_permission(permission: &str) -> bool {
    matches!(permission, "read" | "write" | "full")
}

pub(super) fn sync_lead(config: &mut Config) {
    let lead = &config.mutis.lead;
    config.planner_provider_id = lead.provider_id.clone();
    config.planner_model = lead.model.clone();
    config.mutis.reviewer_provider_id = lead.provider_id.clone();
    config.mutis.reviewer_model = lead.model.clone();
}

/// Applies the endpoint allowlist and the range/enum bounds shared with PUT /api/mutis.
pub fn apply_setting_change(
    config: &mut Config,
    path: &str,
    value: &Value,
) -> Result<(Value, Value), String> {
    let change = apply_change(config, path, value)?;
    if matches!(path, "mutis.lead.provider_id" | "mutis.lead.model") {
        sync_lead(config);
    }
    Ok(change)
}

fn apply_change(
    config: &mut Config,
    path: &str,
    value: &Value,
) -> Result<(Value, Value), String> {
    let m = &mut config.mutis;
    let old: Value;
    let new: Value;
    macro_rules! boolean {
        ($field:ident) => {{
            let v = value.as_bool().ok_or("Giá trị phải là đúng hoặc sai")?;
            old = json!(m.$field);
            m.$field = v;
            new = json!(v);
        }};
    }
    macro_rules! bounded {
        ($($field:ident).+, $min:literal, $max:literal) => {{
            let v = value
                .as_u64()
                .filter(|v| ($min..=$max).contains(v))
                .ok_or("Giá trị ngoài giới hạn")? as _;
            old = json!(m.$($field).+);
            m.$($field).+ = v;
            new = json!(v);
        }};
    }
    macro_rules! text {
        ($field:expr, $check:expr, $error:literal) => {{
            let v = value.as_str().filter($check).ok_or($error)?;
            old = json!($field);
            $field = v.into();
            new = json!(v);
        }};
    }
    let provider_exists = |id: &&str| {
        id.is_empty() || config.providers.iter().any(|provider| provider.id == *id)
    };
    let valid_model = |model: &&str| !model.trim().is_empty() && model.len() <= 200;
    macro_rules! role_effort {
        ($role:ident) => {
            text!(
                m.$role.effort,
                |effort| valid_effort(&m.$role.provider_id, &m.$role.model, effort),
                "Mức suy luận không hợp lệ"
            )
        };
    }
    match path {
        "mutis.retry_api" => boolean!(retry_api),
        "mutis.retry_attempts" => bounded!(retry_attempts, 1, 5),
        "mutis.retry_base_ms" => bounded!(retry_base_ms, 0, 60000),
        "mutis.codex_watchdog" => boolean!(codex_watchdog),
        "mutis.watchdog_minutes" => bounded!(watchdog_minutes, 1, 60),
        "mutis.codex_auto_resume" => boolean!(codex_auto_resume),
        "mutis.resume_max" => bounded!(resume_max, 0, 3),
        "mutis.deepseek_agent" => boolean!(deepseek_agent),
        "mutis.deepseek_max_file_lines" => bounded!(deepseek_max_file_lines, 50, 5000),
        "mutis.deepseek_max_steps" => bounded!(deepseek_max_steps, 5, 40),
        "mutis.debate_rounds" => bounded!(debate_rounds, 1, 3),
        "mutis.verify_gate" => boolean!(verify_gate),
        "mutis.auto_review" => boolean!(auto_review),
        "mutis.auto_fix" => boolean!(auto_fix),
        "mutis.fix_rounds_max" => bounded!(fix_rounds_max, 0, 5),
        "mutis.event_log" => boolean!(event_log),
        "mutis.auto_plan" => boolean!(auto_plan),
        "mutis.plan_approval" => boolean!(plan_approval),
        "mutis.delegate_checker" => boolean!(delegate_checker),
        "mutis.lead.provider_id" => {
            text!(m.lead.provider_id, provider_exists, "Nguồn AI không tồn tại")
        }
        "mutis.lead.model" => text!(m.lead.model, valid_model, "Model không hợp lệ"),
        "mutis.lead.effort" => role_effort!(lead),
        "mutis.worker.provider_id" => {
            text!(m.worker.provider_id, provider_exists, "Nguồn AI không tồn tại")
        }
        "mutis.worker.model" => text!(m.worker.model, valid_model, "Model không hợp lệ"),
        "mutis.worker.effort" => role_effort!(worker),
        "mutis.worker.permission" => {
            text!(m.worker.permission, |p| valid_permission(p), "Quyền không hợp lệ")
        }
        "mutis.worker.count" => bounded!(worker.count, 1, 3),
        "mutis.checker.provider_id" => {
            text!(m.checker.provider_id, provider_exists, "Nguồn AI không tồn tại")
        }
        "mutis.checker.model" => text!(m.checker.model, valid_model, "Model không hợp lệ"),
        "mutis.checker.effort" => role_effort!(checker),
        "mutis.checker.permission" => {
            text!(m.checker.permission, |p| valid_permission(p), "Quyền không hợp lệ")
        }
        _ => return Err("Không được phép đổi thiết lập này".into()),
    }
    Ok((old, new))
}
