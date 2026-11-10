use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_id: Option<String>,
    pub title: String,
    pub folder: String,
    #[serde(default)]
    pub dev_chat: bool,
    pub request: String,
    pub approved: bool,
    #[serde(default)]
    pub planning: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub tasks: Vec<PlanTask>,
    pub updated_at: u64,
    #[serde(default)]
    pub auto_continue: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanTask {
    pub id: u32,
    pub title: String,
    pub detail: String,
    pub assignee: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission: Option<String>,
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
    #[serde(default)]
    pub verify: Option<serde_json::Value>,
    #[serde(default)]
    pub review: Option<serde_json::Value>,
    #[serde(default)]
    pub fix_rounds_used: u32,
    /// For fix tasks: the original task's title/detail, so later rounds keep the real context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_root_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_root_detail: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ProviderSummary {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub models: Vec<String>,
    pub agent_capable: bool,
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
pub fn reassign_with_routing(
    kind: &str,
    connected: &[ProviderSummary],
    planner_id: &str,
    routing: &[crate::config::RoutingRule],
    roles: &crate::config::MutisRoles,
    enabled: bool,
) -> String {
    if enabled {
        if let Some(rule) = routing.iter().find(|rule| rule.kind == kind) {
            let role = match rule.role.as_str() {
                "lead" => &roles.lead,
                "big" => &roles.big,
                "small" => &roles.small,
                _ => &roles.lead,
            };
            if connected
                .iter()
                .any(|provider| provider.id == role.provider_id)
            {
                return role.provider_id.clone();
            }
        }
    }
    reassign(kind, connected, planner_id)
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
        let directory = crate::core::paths::plans_dir(config_path)?;
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
            chat_id: None,
            title,
            folder,
            dev_chat: false,
            request,
            approved: false,
            planning: false,
            error: None,
            tasks,
            updated_at: now(),
            auto_continue: false,
        }
    }
    pub fn chat_summary(&self) -> String {
        let mut text = format!("✦ Chia việc · {}", self.title);
        for task in &self.tasks {
            text.push_str(&format!(
                "\n- [{}] {} · {}",
                task.status, task.title, task.assignee
            ));
        }
        if let Some(error) = &self.error {
            text.push_str(&format!("\nLỗi: {error}"));
        }
        text.chars().take(1500).collect()
    }

    /// Re-read the chat so other turns survive task progress updates.
    pub fn save_chat(
        &self,
        store: &crate::chat::ChatStore,
        append_request: bool,
        planning: bool,
    ) -> Result<()> {
        use crate::chat::{self, ChatRequest};
        let Some(id) = self.chat_id.as_deref() else {
            return Ok(());
        };
        anyhow::ensure!(chat::valid_id(id), "chat_id không hợp lệ");
        let mut record = store.get(id)?.unwrap_or_else(|| {
            chat::new_record(&ChatRequest {
                chat_id: id.into(),
                provider_id: String::new(),
                model: String::new(),
                folder: self.folder.clone(),
                dev_chat: self.dev_chat,
                permission: Default::default(),
                file_tools: Default::default(),
                effort: None,
                messages: Vec::new(),
                plan_id: None,
                task_id: None,
                task_kind: None,
            })
        });
        if record.messages.is_empty() {
            record.title = self.title.clone();
        }
        if planning {
            record.folder = self.folder.clone();
            record.dev_chat = self.dev_chat;
        }
        let content = if planning {
            String::new()
        } else {
            self.chat_summary()
        };
        if let Some(message) = record
            .messages
            .iter_mut()
            .find(|m| m.plan_id.as_deref() == Some(&self.id))
        {
            message.content = content;
        } else {
            if append_request {
                record
                    .messages
                    .push(serde_json::from_value(serde_json::json!({
                        "role": "user", "content": self.request
                    }))?);
            }
            record
                .messages
                .push(serde_json::from_value(serde_json::json!({
                    "role": "assistant", "content": content, "plan_id": self.id
                }))?);
        }
        chat::save_record(store, &mut record)
    }

    pub fn touch(&mut self) {
        self.updated_at = now();
    }
    pub fn with_dev_chat(mut self, dev_chat: bool) -> Self {
        self.dev_chat = dev_chat;
        self
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
            permission: None,
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
            verify: None,
            review: None,
            fix_rounds_used: 0,
            fix_root_title: None,
            fix_root_detail: None,
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
                ..Default::default()
            },
            ProviderSummary {
                id: "openai".into(),
                name: "OpenAI".into(),
                kind: "openai_compat".into(),
                models: vec![],
                ..Default::default()
            },
            ProviderSummary {
                id: "codex".into(),
                name: "Codex".into(),
                kind: "codex_cli".into(),
                models: vec![],
                ..Default::default()
            },
        ];
        assert_eq!(reassign("code", &connected, "planner"), "codex");
        assert_eq!(reassign("test", &connected[..1], "planner"), "claude");
        assert_eq!(reassign("docs", &connected, "planner"), "openai");
        assert_eq!(reassign("review", &connected, "planner"), "planner");
        assert_eq!(reassign("docs", &[], "planner"), "planner");
    }
    #[test]
    fn reassign_uses_mutis_routing_when_enabled() {
        let connected = vec![
            ProviderSummary {
                id: "ds".into(),
                name: "DS".into(),
                kind: "openai_compat".into(),
                models: vec![],
                ..Default::default()
            },
            ProviderSummary {
                id: "codex".into(),
                name: "Codex".into(),
                kind: "codex_cli".into(),
                models: vec![],
                ..Default::default()
            },
        ];
        let routes = vec![crate::config::RoutingRule {
            kind: "code".into(),
            role: "small".into(),
            assignee_kind: "deepseek_agent".into(),
            note: String::new(),
        }];
        let roles = crate::config::MutisRoles {
            small: crate::config::MutisRole {
                provider_id: "ds".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            reassign_with_routing("code", &connected, "p", &routes, &roles, true),
            "ds"
        );
        let unavailable = crate::config::MutisRoles {
            small: crate::config::MutisRole {
                provider_id: "missing".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        assert_eq!(
            reassign_with_routing("code", &connected, "p", &routes, &unavailable, true),
            "codex"
        );
        assert_eq!(
            reassign_with_routing("code", &connected, "p", &routes, &roles, false),
            "codex"
        );
    }
}
