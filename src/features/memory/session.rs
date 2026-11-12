//! One chat = one terse `key: value` file (English). `files:` is filled by the app, never the AI.
use anyhow::{bail, ensure, Result};
use serde::Deserialize;

pub const STATUSES: [&str; 4] = ["active", "blocked", "done", "obsolete"];
const LIST_MAX: usize = 30;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated: String,
    pub goal: String,
    pub decided: Vec<String>,
    pub open: Vec<String>,
    pub done: Vec<String>,
    pub files: Vec<String>,
    pub evidence: Vec<String>,
}

/// What the AI writes in its `<memory>` block.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct Update {
    pub title: String,
    pub status: String,
    pub goal: String,
    #[serde(alias = "decisions")]
    pub decided: Vec<String>,
    pub open: Vec<String>,
    pub done: Vec<String>,
    pub evidence: Vec<String>,
}

pub fn valid_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
pub fn line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
/// Append unique non-empty single-line values, keeping the newest `LIST_MAX`.
pub fn append(to: &mut Vec<String>, from: Vec<String>) {
    let values = std::mem::take(to).into_iter().chain(from);
    for value in values {
        let value = line(&value);
        if !value.is_empty() && !to.contains(&value) {
            to.push(value);
        }
    }
    if to.len() > LIST_MAX {
        to.drain(..to.len() - LIST_MAX);
    }
}

impl Session {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            status: "active".into(),
            ..Default::default()
        }
    }
    pub fn text(&self) -> String {
        self.render(true)
    }
    /// What a recall attaches to a message: finished (`done:`) lines are skipped.
    pub fn loadable(&self) -> String {
        self.render(false)
    }
    fn render(&self, with_done: bool) -> String {
        let mut out = String::new();
        let mut put = |key: &str, value: &str| {
            let value = line(value);
            if !value.is_empty() {
                out.push_str(&format!("{key}: {value}\n"));
            }
        };
        put("title", &self.title);
        put("status", &self.status);
        put("updated", &self.updated);
        put("goal", &self.goal);
        let lists = [
            ("decided", &self.decided, true),
            ("open", &self.open, true),
            ("done", &self.done, with_done),
            ("files", &self.files, true),
            ("evidence", &self.evidence, true),
        ];
        for (key, items, show) in lists {
            if show {
                items.iter().for_each(|v| put(key, v));
            }
        }
        out
    }
    pub fn parse(id: &str, text: &str) -> Result<Self> {
        ensure!(valid_id(id), "Invalid session id");
        let mut s = Self::new(id);
        s.status.clear();
        for row in text.replace("\r\n", "\n").lines() {
            let Some((key, value)) = row.split_once(':') else {
                continue;
            };
            let value = value.trim().to_owned();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "title" => s.title = value,
                "status" => s.status = value,
                "updated" => s.updated = value,
                "goal" => s.goal = value,
                "decided" => s.decided.push(value),
                "open" => s.open.push(value),
                "done" => s.done.push(value),
                "files" => s.files.push(value),
                "evidence" => s.evidence.push(value),
                _ => {}
            }
        }
        if !STATUSES.contains(&s.status.as_str()) {
            bail!("Invalid status");
        }
        Ok(s)
    }
    pub fn retired(&self) -> bool {
        matches!(self.status.as_str(), "done" | "obsolete")
    }
    /// Drop the least valuable line; false when nothing is left to drop.
    pub fn shrink(&mut self) -> bool {
        for l in [&mut self.done, &mut self.files, &mut self.decided, &mut self.open] {
            if !l.is_empty() {
                l.remove(0);
                return true;
            }
        }
        if self.evidence.len() > 1 {
            self.evidence.remove(0);
            return true;
        }
        for t in [&mut self.goal, &mut self.title] {
            let n = t.chars().count();
            if n > 20 {
                *t = t.chars().take(n / 2).collect();
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Session {
        Session {
            id: "chat-1".into(),
            title: "Fix: login \"bug\" C:\\x".into(),
            status: "blocked".into(),
            updated: "2026-11-12T00:00:00Z".into(),
            goal: "Ship it".into(),
            decided: vec!["use A".into()],
            open: vec!["t1".into(), "t2".into()],
            done: vec!["old work".into()],
            files: vec!["src/a.rs".into()],
            evidence: vec!["abc123".into()],
        }
    }
    #[test]
    fn roundtrip() {
        let s = sample();
        assert_eq!(Session::parse("chat-1", &s.text()).unwrap(), s);
    }
    #[test]
    fn loadable_skips_done_lines() {
        let l = sample().loadable();
        assert!(!l.contains("old work") && !l.contains("done:"));
        assert!(l.contains("open: t2"));
    }
    #[test]
    fn rejects_bad_input() {
        assert!(Session::parse("../x", "status: active").is_err());
        assert!(Session::parse("x", "status: nope").is_err());
        assert!(Session::parse("x", "title: no status").is_err());
    }
}
