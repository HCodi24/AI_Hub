use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub kind: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub assignee_kind: String,
    #[serde(default)]
    pub note: String,
}
