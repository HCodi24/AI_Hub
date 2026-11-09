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
