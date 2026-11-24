use serde_json::{json, Value};

/// Applies the endpoint allowlist and the range/enum bounds shared with PUT /api/mutis.
pub fn apply_setting_change(
    config: &mut crate::config::Config,
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
        ($field:ident, $min:literal, $max:literal) => {{
            let v = value
                .as_u64()
                .filter(|v| ($min..=$max).contains(v))
                .ok_or("Giá trị ngoài giới hạn")? as _;
            old = json!(m.$field);
            m.$field = v;
            new = json!(v);
        }};
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
        "mutis.deepseek_ui" => boolean!(deepseek_ui),
        _ if path.starts_with("mutis.roles.") => return apply_role_change(config, path, value),
        _ if path.starts_with("mutis.routing[") && path.ends_with("].role") => {
            return apply_routing_change(config, path, value);
        }
        _ => return Err("Không được phép đổi thiết lập này".into()),
    }
    Ok((old, new))
}

fn apply_role_change(
    config: &mut crate::config::Config,
    path: &str,
    value: &Value,
) -> Result<(Value, Value), String> {
    let m = &mut config.mutis;
    let old: Value;
    let new: Value;
    let mut parts = path.split('.');
    let _ = parts.next();
    let _ = parts.next();
    let role_name = parts.next().unwrap_or_default();
    let field = parts.next().unwrap_or_default();
    if parts.next().is_some() {
        return Err("Không được phép đổi thiết lập này".into());
    }
    let role = match role_name {
        "lead" => &mut m.roles.lead,
        "big" => &mut m.roles.big,
        "small" => &mut m.roles.small,
        _ => return Err("Không được phép đổi thiết lập này".into()),
    };
    match field {
        "provider_id" => {
            let v = value
                .as_str()
                .filter(|v| config.providers.iter().any(|p| p.id == *v))
                .ok_or("Không tìm thấy nguồn AI")?;
            old = json!(role.provider_id);
            role.provider_id = v.into();
            new = json!(v);
        }
        "model" => {
            let v = value
                .as_str()
                .filter(|v| !v.trim().is_empty() && v.len() <= 200)
                .ok_or("Model không hợp lệ")?;
            old = json!(role.model);
            role.model = v.into();
            new = json!(v);
        }
        "effort" => {
            let v = value
                .as_str()
                .filter(|v| {
                    matches!(*v, "default" | "low" | "medium" | "high" | "xhigh" | "max")
                })
                .ok_or("Mức suy luận không hợp lệ")?;
            old = json!(role.effort);
            role.effort = v.into();
            new = json!(v);
        }
        _ => return Err("Không được phép đổi thiết lập này".into()),
    }
    Ok((old, new))
}

fn apply_routing_change(
    config: &mut crate::config::Config,
    path: &str,
    value: &Value,
) -> Result<(Value, Value), String> {
    let m = &mut config.mutis;
    let index = path
        .strip_prefix("mutis.routing[")
        .and_then(|p| p.strip_suffix("].role"))
        .and_then(|i| i.parse::<usize>().ok())
        .ok_or("Không được phép đổi thiết lập này")?;
    let rule = m
        .routing
        .get_mut(index)
        .ok_or("Không tìm thấy quy tắc định tuyến")?;
    let role = value
        .as_str()
        .filter(|v| matches!(*v, "lead" | "big" | "small"))
        .ok_or("Vai trò không hợp lệ")?;
    let old = json!(rule.role);
    rule.role = role.into();
    Ok((old, json!(role)))
}
