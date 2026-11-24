use serde::{Deserialize, Serialize};

use super::defaults::default_effort;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRoles {
    pub lead: MutisRole,
    pub big: MutisRole,
    pub small: MutisRole,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRole {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
}
