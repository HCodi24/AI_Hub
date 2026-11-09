use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub ts: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    pub provider_id: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_kind: Option<String>,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

pub fn normalize_error(value: &str) -> String {
    let mut out = String::new();
    let mut digits = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() {
            if !digits {
                out.push('#');
            }
            digits = true;
        } else {
            digits = false;
            out.push(ch);
        }
    }
    out.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(180)
        .collect()
}

#[allow(dead_code)]
pub fn parse_diagnosis(text: &str) -> anyhow::Result<serde_json::Value> {
    let start = text
        .find('{')
        .ok_or_else(|| anyhow::anyhow!("Thiếu JSON"))?;
    let mut depth = 0;
    for (offset, ch) in text[start..].char_indices() {
        if ch == '{' {
            depth += 1;
        }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                let value: serde_json::Value =
                    serde_json::from_str(&text[start..start + offset + 1])?;
                if !matches!(value["health"].as_str(), Some("tốt" | "trung bình" | "kém"))
                    || !value["findings"].is_array()
                {
                    anyhow::bail!("Sai cấu trúc chẩn đoán");
                }
                return validate_diagnosis(value);
            }
        }
    }
    anyhow::bail!("JSON chưa hoàn chỉnh")
}

pub fn validate_diagnosis(value: Value) -> anyhow::Result<Value> {
    if !matches!(value["health"].as_str(), Some("tốt" | "trung bình" | "kém"))
        || !value["findings"].is_array()
        || !value["summary"].is_string()
    {
        anyhow::bail!("Invalid diagnosis shape");
    }
    let findings = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let field = |name: &str| {
                finding[name]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid finding field: {name}"))
            };
            let severity = field("severity")?;
            if !matches!(severity, "high" | "medium" | "low") {
                anyhow::bail!("Invalid finding severity");
            }
            let setting_change_value = finding
                .get("setting_change")
                .ok_or_else(|| anyhow::anyhow!("Missing setting_change"))?;
            let setting_change = match setting_change_value {
                Value::Null => Value::Null,
                change if change["path"].is_string() && change.get("value").is_some() => {
                    json!({"path": change["path"], "value": change["value"]})
                }
                _ => anyhow::bail!("Invalid setting_change shape"),
            };
            Ok(json!({
                "problem": field("problem")?,
                "evidence": field("evidence")?,
                "cause": field("cause")?,
                "severity": severity,
                "suggestion": field("suggestion")?,
                "setting_change": setting_change
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(json!({"health": value["health"], "findings": findings, "summary": value["summary"]}))
}

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
            let v = value.as_bool().ok_or("Expected boolean")?;
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
                .ok_or("Value out of range")? as _;
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
        "mutis.verify_gate" => boolean!(verify_gate),
        "mutis.auto_review" => boolean!(auto_review),
        "mutis.auto_fix" => boolean!(auto_fix),
        "mutis.fix_rounds_max" => bounded!(fix_rounds_max, 0, 5),
        "mutis.event_log" => boolean!(event_log),
        _ if path.starts_with("mutis.roles.") => {
            let mut parts = path.split('.');
            let _ = parts.next();
            let _ = parts.next();
            let role_name = parts.next().unwrap_or_default();
            let field = parts.next().unwrap_or_default();
            if parts.next().is_some() {
                return Err("Path is not allowed".into());
            }
            let role = match role_name {
                "lead" => &mut m.roles.lead,
                "big" => &mut m.roles.big,
                "small" => &mut m.roles.small,
                _ => return Err("Path is not allowed".into()),
            };
            match field {
                "provider_id" => {
                    let v = value
                        .as_str()
                        .filter(|v| config.providers.iter().any(|p| p.id == *v))
                        .ok_or("Provider does not exist")?;
                    old = json!(role.provider_id);
                    role.provider_id = v.into();
                    new = json!(v);
                }
                "model" => {
                    let v = value
                        .as_str()
                        .filter(|v| !v.trim().is_empty() && v.len() <= 200)
                        .ok_or("Invalid model")?;
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
                        .ok_or("Invalid effort")?;
                    old = json!(role.effort);
                    role.effort = v.into();
                    new = json!(v);
                }
                _ => return Err("Path is not allowed".into()),
            }
        }
        _ if path.starts_with("mutis.routing[") && path.ends_with("].role") => {
            let index = path
                .strip_prefix("mutis.routing[")
                .and_then(|p| p.strip_suffix("].role"))
                .and_then(|i| i.parse::<usize>().ok())
                .ok_or("Path is not allowed")?;
            let rule = m
                .routing
                .get_mut(index)
                .ok_or("Routing rule does not exist")?;
            let role = value
                .as_str()
                .filter(|v| matches!(*v, "lead" | "big" | "small"))
                .ok_or("Invalid role")?;
            old = json!(rule.role);
            rule.role = role.into();
            new = json!(role);
        }
        _ => return Err("Path is not allowed".into()),
    }
    Ok((old, new))
}
#[derive(Clone)]
pub struct EventLog {
    directory: PathBuf,
}
impl EventLog {
    pub fn new(config_path: &std::path::Path) -> anyhow::Result<Self> {
        let directory = config_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Config path has no parent"))?
            .join("mutis");
        fs::create_dir_all(&directory)?;
        let old = config_path
            .parent()
            .unwrap()
            .join("orchestration")
            .join("events.jsonl");
        let new = directory.join("events.jsonl");
        if old.exists() && !new.exists() {
            fs::rename(old, new)?;
        }
        Ok(Self { directory })
    }
    pub fn append(&self, event: &Event) -> anyhow::Result<()> {
        let path = self.directory.join("events.jsonl");
        if fs::metadata(&path)
            .map(|m| m.len() >= 5 * 1024 * 1024)
            .unwrap_or(false)
        {
            let rotated = self.directory.join("events.1.jsonl");
            let _ = fs::remove_file(&rotated);
            fs::rename(&path, rotated)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(event)?)?;
        Ok(())
    }
    pub fn recent(&self, limit: usize) -> anyhow::Result<Vec<Event>> {
        let limit = limit.min(2000);
        let current = self.directory.join("events.jsonl");
        let rotated = self.directory.join("events.1.jsonl");
        let mut events = Vec::new();
        for path in [current, rotated] {
            if events.len() >= limit {
                break;
            }
            if let Ok(data) = fs::read_to_string(path) {
                events.extend(
                    data.lines()
                        .rev()
                        .take(limit - events.len())
                        .filter_map(|line| serde_json::from_str(line).ok()),
                );
            }
        }
        Ok(events)
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod diagnosis_tests {
    use super::{apply_setting_change, normalize_error, parse_diagnosis, validate_diagnosis};
    use serde_json::{json, Value};

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
        apply_setting_change(&mut config, "mutis.roles.big.effort", &json!("high")).unwrap();
        apply_setting_change(&mut config, "mutis.routing[2].role", &json!("small")).unwrap();
        for path in [
            "mutis.roles.lead.api_key",
            "providers[0].api_key",
            "mutis.unknown",
            "mutis.verify_command",
            "mutis.worker_rules",
            "mutis.paths",
        ] {
            assert!(
                apply_setting_change(&mut config, path, &json!("x")).is_err(),
                "{path}"
            );
        }
        assert!(apply_setting_change(&mut config, "mutis.watchdog_minutes", &json!(61)).is_err());
        assert!(apply_setting_change(&mut config, "mutis.retry_attempts", &json!(0)).is_err());
        assert!(apply_setting_change(
            &mut config,
            "mutis.roles.big.provider_id",
            &json!("missing")
        )
        .is_err());
        assert!(apply_setting_change(&mut config, "mutis.routing[2].role", &Value::Null).is_err());
    }
}
