use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexConfig {
    pub project_skills: bool,
    pub project_instructions: bool,
    /// Invalidates runtime sessions when source permissions or authentication change.
    pub revision: u64,
}
