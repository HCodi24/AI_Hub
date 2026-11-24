use serde::{Deserialize, Deserializer, Serialize};

use super::defaults::{default_effort, default_permission, default_worker_count};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MutisRole {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutisWorker {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default = "default_permission", deserialize_with = "deserialize_permission")]
    pub permission: String,
    #[serde(default = "default_worker_count")]
    pub count: u32,
}

impl Default for MutisWorker {
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            model: String::new(),
            effort: default_effort(),
            permission: default_permission(),
            count: default_worker_count(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MutisChecker {
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub model: String,
    #[serde(default = "default_effort")]
    pub effort: String,
    #[serde(default = "default_permission", deserialize_with = "deserialize_permission")]
    pub permission: String,
}

impl Default for MutisChecker {
    fn default() -> Self {
        Self {
            provider_id: String::new(),
            model: String::new(),
            effort: default_effort(),
            permission: default_permission(),
        }
    }
}

fn deserialize_permission<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let permission = String::deserialize(deserializer)?;
    match permission.as_str() {
        "read" | "write" | "full" => Ok(permission),
        _ => Err(serde::de::Error::unknown_variant(
            &permission,
            &["read", "write", "full"],
        )),
    }
}
