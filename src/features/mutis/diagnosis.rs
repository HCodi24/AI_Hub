use super::{build_stats, Event};
use crate::{
    api::providers::cached_provider_test,
    app::AppState,
    chat::{self, ChatMessage},
    config::{Config, ProviderConfig},
    providers::{self, ProviderEvent, ProviderRequest},
};
use serde_json::{json, Value};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
        anyhow::bail!("Cấu trúc kết quả chẩn đoán không hợp lệ");
    }
    let findings = value["findings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| {
            let field = |name: &str| {
                finding[name]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Trường phát hiện không hợp lệ: {name}"))
            };
            let severity = field("severity")?;
            if !matches!(severity, "high" | "medium" | "low") {
                anyhow::bail!("Mức độ nghiêm trọng của phát hiện không hợp lệ");
            }
            let setting_change_value = finding
                .get("setting_change")
                .ok_or_else(|| anyhow::anyhow!("Thiếu thay đổi thiết lập"))?;
            let setting_change = match setting_change_value {
                Value::Null => Value::Null,
                change if change["path"].is_string() && change.get("value").is_some() => {
                    json!({"path": change["path"], "value": change["value"]})
                }
                _ => anyhow::bail!("Cấu trúc thay đổi thiết lập không hợp lệ"),
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

pub fn list_diagnoses(state: &AppState) -> Vec<Value> {
    let directory = diagnoses_dir(state);
    let mut items: Vec<Value> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            if let Ok(data) = std::fs::read_to_string(entry.path()) {
                if let Ok(value) = serde_json::from_str(&data) {
                    items.push(value);
                }
            }
        }
    }
    items.sort_by_key(|v| std::cmp::Reverse(v["ts"].as_u64().unwrap_or(0)));
    items
}

pub enum DiagnosisRun {
    Complete(Value),
    Running {
        receiver: mpsc::Receiver<ProviderEvent>,
        provider_id: String,
        ts: u64,
    },
}

pub enum DiagnosisError {
    ReadEvents,
    ReadStats,
    Save(&'static str),
    NoConnectedLead(&'static str),
    InvalidProviderKind,
}

pub async fn start_diagnosis(state: &AppState) -> Result<DiagnosisRun, DiagnosisError> {
    let all_events = state
        .event_log
        .recent(2000)
        .map_err(|_| DiagnosisError::ReadEvents)?;
    let ts = crate::core::time::now_secs();
    let insufficient = all_events.len() < 5;
    let config_snapshot = state.config.read().await.clone();
    let failures = failure_events(&all_events);
    let stats = build_stats(&state.event_log, 30).map_err(|_| DiagnosisError::ReadStats)?;
    if insufficient {
        let diagnosis = json!({"ts":ts,"health":"chưa đủ dữ liệu","findings":[],"summary":"Chưa đủ dữ liệu để chẩn đoán"});
        save_and_record_diagnosis(state, ts, String::new(), &diagnosis)
            .map_err(DiagnosisError::Save)?;
        return Ok(DiagnosisRun::Complete(diagnosis));
    }
    let (provider_config, model, effort) = resolve_connected_lead(state, &config_snapshot)
        .await
        .map_err(DiagnosisError::NoConnectedLead)?;
    let provider = providers::for_kind(&provider_config.kind)
        .map_err(|_| DiagnosisError::InvalidProviderKind)?;
    let settings = serde_json::to_value(&config_snapshot.mutis).unwrap_or(Value::Null);
    let prompt = diagnosis_prompt(&settings, &stats, &failures);
    let request = ProviderRequest {
        provider: provider_config.clone(),
        model,
        folder: config_snapshot.default_folder.clone(),
        permission: chat::Permission::Read,
        effort,
        messages: vec![ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "user".into(),
            content: prompt,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: String::new(),
            model: String::new(),
            error: None,
        }],
        session_id: None,
        mutis: config_snapshot.mutis.clone(),
        mcp: Vec::new(),
        write_scope: Vec::new(),
        checker_session_id: None,
    };
    let (sender, receiver) = mpsc::channel::<ProviderEvent>(32);
    tokio::spawn(async move {
        if let Err(error) = provider
            .run(request, sender.clone(), CancellationToken::new())
            .await
        {
            let _ = sender
                .send(ProviderEvent::text("error", error.to_string()))
                .await;
        }
    });
    Ok(DiagnosisRun::Running {
        receiver,
        provider_id: provider_config.id,
        ts,
    })
}

pub fn finish_diagnosis(
    state: &AppState,
    ts: u64,
    provider_id: String,
    raw: &str,
) -> Result<Value, &'static str> {
    let value = crate::core::text::extract_json(raw)
        .and_then(|text| serde_json::from_str::<Value>(text).ok())
        .and_then(|value| validate_diagnosis(value).ok())
        .ok_or("Không phân tích được kết quả chẩn đoán JSON")?;
    let diagnosis = json!({"ts":ts,"health":value["health"],"findings":value["findings"],"summary":value["summary"]});
    save_and_record_diagnosis(state, ts, provider_id, &diagnosis)?;
    Ok(diagnosis)
}

fn save_and_record_diagnosis(
    state: &AppState,
    ts: u64,
    provider_id: String,
    diagnosis: &Value,
) -> Result<(), &'static str> {
    save_diagnosis(state, diagnosis)?;
    let _ = state.event_log.append(&Event {
        ts,
        plan_id: None,
        task_id: None,
        provider_id,
        kind: "diagnosis".into(),
        task_kind: None,
        ok: true,
        duration_ms: None,
        tokens: None,
        detail: Some(diagnosis["summary"].as_str().unwrap_or_default().into()),
    });
    Ok(())
}

pub(super) fn failure_events(all: &[Event]) -> Vec<Event> {
    let mut failure_events = all
        .iter()
        .filter(|event| {
            matches!(
                event.kind.as_str(),
                "retry" | "watchdog_kill" | "resume" | "fix_task" | "error"
            ) || (event.kind == "verify" && !event.ok)
                || (event.kind == "review" && !event.ok)
                || event
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.to_lowercase().contains("empty deepseek"))
        })
        .take(150)
        .cloned()
        .collect::<Vec<_>>();
    for event in &mut failure_events {
        if let Some(detail) = &mut event.detail {
            *detail = detail.chars().take(300).collect();
        }
    }
    failure_events
}

pub(super) fn diagnosis_prompt(settings: &Value, stats: &Value, failures: &[Event]) -> String {
    format!(
        concat!(
            "You are the read-only MutiS reviewer. Analyze the provided settings, stats ",
            "and recent failure events. Return ONLY JSON matching this contract exactly: ",
            "{{\"health\":\"tốt|trung bình|kém\",\"findings\":[",
            "{{\"problem\":\"\",\"evidence\":\"\",\"cause\":\"\",",
            "\"severity\":\"high|medium|low\",\"suggestion\":\"\",",
            "\"setting_change\":{{\"path\":\"mutis.<field> or ",
            "mutis.lead.<provider_id|model|effort> or ",
            "mutis.worker.<provider_id|model|effort|permission|count> or ",
            "mutis.checker.<provider_id|model|effort|permission>\",\"value\":null}} or null}}],",
            "\"summary\":\"\"}}. Do not include unknown fields or propose secrets, ",
            "provider keys, paths, verify commands or worker rules.\n",
            "SETTINGS (no provider keys):\n{}\nSTATS:\n{}\n",
            "LAST FAILURE EVENTS (max 150, each detail max 300 chars):\n{}"
        ),
        settings,
        stats,
        serde_json::to_string(failures).unwrap_or_default()
    )
}

fn diagnoses_dir(state: &AppState) -> PathBuf {
    state
        .config_path
        .parent()
        .unwrap()
        .join("mutis")
        .join("diagnoses")
}

fn save_diagnosis(state: &AppState, diagnosis: &Value) -> Result<(), &'static str> {
    let dir = diagnoses_dir(state);
    std::fs::create_dir_all(&dir).map_err(|_| "Không lưu được chẩn đoán")?;
    std::fs::write(
        dir.join(format!(
            "{}.json",
            diagnosis["ts"].as_u64().unwrap_or_default()
        )),
        serde_json::to_vec_pretty(diagnosis).unwrap(),
    )
    .map_err(|_| "Không lưu được chẩn đoán")
}

async fn resolve_connected_lead(
    state: &AppState,
    config: &Config,
) -> Result<(ProviderConfig, String, chat::Effort), &'static str> {
    let role = &config.mutis.lead;
    let mut candidates = Vec::new();
    if let Some(provider) = config.providers.iter().find(|p| p.id == role.provider_id) {
        candidates.push(provider.clone());
    }
    for kind in ["claude_cli", "anthropic", "codex_cli"] {
        for provider in config.providers.iter().filter(|p| p.kind == kind) {
            if !candidates
                .iter()
                .any(|candidate| candidate.id == provider.id)
            {
                candidates.push(provider.clone());
            }
        }
    }
    for provider in candidates {
        if cached_provider_test(state, &provider.id)
            .await
            .is_ok_and(|status| status.status == "connected")
        {
            let model = if role.provider_id == provider.id && !role.model.is_empty() {
                role.model.clone()
            } else {
                provider.models.first().cloned().unwrap_or_default()
            };
            if model.is_empty() {
                continue;
            }
            let effort = chat::parse_effort(if role.provider_id == provider.id {
                &role.effort
            } else {
                "default"
            })
            .unwrap_or_default();
            return Ok((provider, model, effort));
        }
    }
    Err("Không có nguồn đang kết nối để chẩn đoán")
}
