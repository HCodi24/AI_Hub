use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub title: String,
    pub folder: String,
    pub request: String,
    pub approved: bool,
    pub tasks: Vec<PlanTask>,
    pub updated_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: u32,
    pub title: String,
    pub detail: String,
    pub assignee: String,
    #[serde(default)]
    pub depends_on: Vec<u32>,
    pub kind: String,
    #[serde(default = "waiting")]
    pub status: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub log: Vec<String>,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub chat_id: Option<String>,
}
fn waiting() -> String {
    "chờ".into()
}

#[derive(Clone)]
pub struct PlanStore {
    directory: PathBuf,
}
impl PlanStore {
    pub fn new(config_path: &Path) -> Result<Self> {
        let directory = config_path
            .parent()
            .context("Config path has no parent")?
            .join("plans");
        Ok(Self { directory })
    }
    pub fn list(&self) -> Result<Vec<Plan>> {
        if !self.directory.exists() {
            return Ok(Vec::new());
        }
        let mut plans = Vec::new();
        for entry in fs::read_dir(&self.directory)? {
            let path = entry?.path();
            if path.extension().and_then(|v| v.to_str()) != Some("json") {
                continue;
            }
            if let Ok(plan) = fs::read_to_string(path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok())
                .ok_or(())
            {
                plans.push(plan);
            }
        }
        plans.sort_by_key(|p: &Plan| std::cmp::Reverse(p.updated_at));
        Ok(plans)
    }
    pub fn get(&self, id: &str) -> Result<Option<Plan>> {
        let path = self.directory.join(format!("{id}.json"));
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_slice(&fs::read(path)?)?))
    }
    pub fn save(&self, plan: &Plan) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        fs::write(
            self.directory.join(format!("{}.json", plan.id)),
            serde_json::to_vec_pretty(plan)?,
        )?;
        Ok(())
    }
}
impl Plan {
    pub fn new(title: String, folder: String, request: String, tasks: Vec<PlanTask>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            title,
            folder,
            request,
            approved: false,
            tasks,
            updated_at: now(),
        }
    }
    pub fn touch(&mut self) {
        self.updated_at = now();
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
