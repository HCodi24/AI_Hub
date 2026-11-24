use super::types::Debate;
use anyhow::{Context, Result};
use std::{fs, path::PathBuf};

#[derive(Clone)]
pub struct DebateStore {
    directory: PathBuf,
}

impl DebateStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id).context("id tranh luận không hợp lệ")?;
        Ok(self.directory.join(format!("{id}.json")))
    }
    pub fn save(&self, debate: &Debate) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        let json = serde_json::to_string_pretty(debate)?;
        fs::write(self.path(&debate.id)?, json)?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Option<Debate>> {
        let path = self.path(id)?;
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }
    pub fn list(&self) -> Result<Vec<Debate>> {
        let mut debates = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(debate) = serde_json::from_str::<Debate>(&text) {
                        debates.push(debate);
                    }
                }
            }
        }
        debates.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(debates)
    }
}
