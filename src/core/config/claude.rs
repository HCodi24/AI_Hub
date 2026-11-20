use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClaudeConfig {
    pub project_skills: bool,
    pub project_instructions: bool,
    pub revision: u64,
    /// Hub MCP servers (in AI Hub's Claude `.claude.json`) the user switched on; every other one stays off.
    pub enabled_mcp: Vec<String>,
}
