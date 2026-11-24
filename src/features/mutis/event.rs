use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
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

#[derive(Clone)]
pub struct EventLog {
    directory: PathBuf,
}

#[derive(Deserialize)]
pub struct PlanEventInput {
    plan_id: String,
    task_id: String,
    provider_id: String,
    kind: String,
    #[serde(default)]
    task_kind: Option<String>,
    ok: bool,
    detail: Option<String>,
}

pub enum PlanEventError {
    InvalidKind(&'static str),
    AppendFailed,
}

pub fn record_plan_event(log: &EventLog, input: PlanEventInput) -> Result<(), PlanEventError> {
    if !matches!(input.kind.as_str(), "review" | "fix_task" | "verify") {
        return Err(PlanEventError::InvalidKind("Loại sự kiện không hợp lệ"));
    }
    log.append(&Event {
        ts: crate::core::time::now_secs(),
        plan_id: Some(input.plan_id),
        task_id: Some(input.task_id),
        provider_id: input.provider_id,
        kind: input.kind,
        task_kind: input.task_kind,
        ok: input.ok,
        duration_ms: None,
        tokens: None,
        detail: input.detail,
    })
    .map_err(|_| PlanEventError::AppendFailed)
}

impl EventLog {
    pub fn new(config_path: &std::path::Path) -> anyhow::Result<Self> {
        let directory = crate::core::paths::mutis_dir(config_path)?;
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
