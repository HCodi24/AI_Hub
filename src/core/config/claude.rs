use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClaudeConfig {
    pub project_skills: bool,
    pub project_instructions: bool,
    pub revision: u64,
}
