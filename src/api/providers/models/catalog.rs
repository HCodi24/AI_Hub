use crate::config::ModelEffort;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Default)]
pub(super) struct ModelCatalog {
    pub(super) ids: Vec<String>,
    pub(super) efforts: BTreeMap<String, ModelEffort>,
}

impl ModelCatalog {
    pub(super) fn add_codex(&mut self, row: &Value) {
        if row["hidden"].as_bool().unwrap_or(false) {
            return;
        }
        let Some(id) = row["id"].as_str().or_else(|| row["model"].as_str()).map(str::trim)
            .filter(|id| !id.is_empty()) else { return };
        if !self.ids.iter().any(|known| known == id) {
            self.ids.push(id.to_owned());
        }
        let mut supported = Vec::new();
        for effort in row["supportedReasoningEfforts"].as_array().into_iter().flatten()
            .filter_map(|entry| entry["reasoningEffort"].as_str())
            .map(str::trim).filter(|effort| !effort.is_empty())
        {
            if !supported.iter().any(|known| known == effort) {
                supported.push(effort.to_owned());
            }
        }
        let default = row["defaultReasoningEffort"].as_str().map(str::trim)
            .filter(|effort| !effort.is_empty()).map(str::to_owned);
        if row["supportedReasoningEfforts"].is_array() || default.is_some() {
            self.efforts.entry(id.to_owned()).or_insert(ModelEffort { supported, default });
        }
    }

    pub(super) fn add_claude(&mut self, row: &Value) {
        if row["disabled"].as_bool() == Some(true) {
            return;
        }
        // Merge aliases into the same catalog entry as their resolved model.
        let id = row["resolvedModel"].as_str().map(str::trim).filter(|id| !id.is_empty())
            .or_else(|| row["value"].as_str().map(str::trim));
        let Some(id) = id.filter(|id| !id.is_empty() && *id != "default") else { return };
        if !self.ids.iter().any(|known| known == id) {
            self.ids.push(id.to_owned());
        }
        if row["supportsEffort"].as_bool() == Some(false) {
            return;
        }
        let Some(levels) = row["supportedEffortLevels"].as_array() else { return };
        let metadata = self.efforts.entry(id.to_owned()).or_insert_with(|| ModelEffort {
            supported: Vec::new(),
            default: None,
        });
        for level in levels.iter().filter_map(Value::as_str).map(str::trim).filter(|level| !level.is_empty()) {
            if !metadata.supported.iter().any(|known| known == level) {
                metadata.supported.push(level.to_owned());
            }
        }
    }
}
