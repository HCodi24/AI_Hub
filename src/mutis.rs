use serde::{Deserialize, Serialize};
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
                return Ok(value);
            }
        }
    }
    anyhow::bail!("JSON chưa hoàn chỉnh")
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
    use super::{normalize_error, parse_diagnosis};

    #[test]
    fn parses_diagnosis_json_and_normalizes_numbered_errors() {
        let parsed = parse_diagnosis(
            "```json\n{\"health\":\"tốt\",\"findings\":[],\"summary\":\"Ổn\"}\n```",
        )
        .unwrap();
        assert_eq!(parsed["health"], "tốt");
        assert_eq!(
            normalize_error("HTTP 503 on attempt 12"),
            "HTTP # on attempt #"
        );
    }
}
