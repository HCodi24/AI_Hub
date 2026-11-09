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
    pub model: Option<String>,
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default)]
    pub reassigned_from: Option<String>,
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

#[derive(Clone, Debug)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub models: Vec<String>,
}

pub fn reassign(kind: &str, connected: &[ProviderSummary], planner_id: &str) -> String {
    let preferred: &[&str] = match kind {
        "code" | "test" => &["codex_cli", "claude_cli"],
        "docs" => &["openai_compat", "codex_cli"],
        _ => return planner_id.to_owned(),
    };
    preferred
        .iter()
        .find_map(|kind| connected.iter().find(|provider| provider.kind == *kind))
        .map(|provider| provider.id.clone())
        .unwrap_or_else(|| planner_id.to_owned())
}
fn default_effort() -> String {
    "default".into()
}

pub fn has_dependency_cycle(tasks: &[PlanTask]) -> bool {
    fn visit(
        id: u32,
        by_id: &std::collections::HashMap<u32, &PlanTask>,
        visiting: &mut std::collections::HashSet<u32>,
        done: &mut std::collections::HashSet<u32>,
    ) -> bool {
        if done.contains(&id) {
            return false;
        }
        if !visiting.insert(id) {
            return true;
        }
        if let Some(task) = by_id.get(&id) {
            for dep in &task.depends_on {
                if by_id.contains_key(dep) && visit(*dep, by_id, visiting, done) {
                    return true;
                }
            }
        }
        visiting.remove(&id);
        done.insert(id);
        false
    }
    let by_id = tasks
        .iter()
        .map(|t| (t.id, t))
        .collect::<std::collections::HashMap<_, _>>();
    let (mut visiting, mut done) = (
        std::collections::HashSet::new(),
        std::collections::HashSet::new(),
    );
    by_id
        .keys()
        .any(|id| visit(*id, &by_id, &mut visiting, &mut done))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: u32, depends_on: Vec<u32>) -> PlanTask {
        PlanTask {
            id,
            title: "task".into(),
            detail: "detail".into(),
            assignee: "worker".into(),
            model: None,
            effort: "default".into(),
            reassigned_from: None,
            depends_on,
            kind: "code".into(),
            status: waiting(),
            summary: String::new(),
            log: Vec::new(),
            tokens: 0,
            chat_id: None,
        }
    }

    #[test]
    fn dependency_cycle_detection_handles_valid_and_cyclic_graphs() {
        assert!(!has_dependency_cycle(&[
            task(1, vec![]),
            task(2, vec![1]),
            task(3, vec![2])
        ]));
        assert!(has_dependency_cycle(&[task(1, vec![2]), task(2, vec![1])]));
        assert!(has_dependency_cycle(&[task(1, vec![1])]));
    }

    #[test]
    fn reassign_uses_kind_priority_and_planner_fallback() {
        let connected = vec![
            ProviderSummary {
                id: "claude".into(),
                name: "Claude".into(),
                kind: "claude_cli".into(),
                models: vec![],
            },
            ProviderSummary {
                id: "openai".into(),
                name: "OpenAI".into(),
                kind: "openai_compat".into(),
                models: vec![],
            },
            ProviderSummary {
                id: "codex".into(),
                name: "Codex".into(),
                kind: "codex_cli".into(),
                models: vec![],
            },
        ];
        assert_eq!(reassign("code", &connected, "planner"), "codex");
        assert_eq!(reassign("test", &connected[..1], "planner"), "claude");
        assert_eq!(reassign("docs", &connected, "planner"), "openai");
        assert_eq!(reassign("review", &connected, "planner"), "planner");
        assert_eq!(reassign("docs", &[], "planner"), "planner");
    }
}
