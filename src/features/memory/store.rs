//! `memory/<slug>-<6hex>/{core.md,<chat-id>.md}` plus `memory/projects.txt`.
use super::{
    core_md, migrate,
    session::{append, line, valid_id, Session, Update},
    MemoryConfig, STORE_LOCK,
};
use anyhow::{bail, ensure, Result};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub struct Store {
    dir: PathBuf,
    pub project: String,
    cfg: MemoryConfig,
}
pub struct Overview {
    pub project: String,
    pub core: String,
    pub sessions: Vec<Session>,
    pub archived: Vec<Session>,
}
#[derive(Serialize)]
pub struct Change {
    pub scope: String,
    pub status: String,
    pub action: String,
}
#[derive(Serialize)]
pub struct Project {
    pub slug: String,
    pub path: String,
    pub description: String,
}

pub fn project_key(folder: &Path) -> String {
    let path = crate::core::text::strip_verbatim(folder.to_path_buf())
        .to_string_lossy()
        .to_lowercase();
    ring::digest::digest(&ring::digest::SHA256, path.as_bytes()).as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn slugify(folder: &Path) -> String {
    let name = folder.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
    let slug: String = name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    let slug = slug.split('-').filter(|p| !p.is_empty()).collect::<Vec<_>>().join("-");
    let slug: String = slug.chars().take(32).collect();
    if slug.is_empty() { "project".into() } else { slug }
}
/// (memory root, project dir name, old-layout key, display path) without touching the disk.
fn locate(config: &Path, folder: &str) -> Result<(PathBuf, String, String, String)> {
    let folder = fs::canonicalize(folder)?;
    ensure!(folder.is_dir(), "Folder does not exist");
    let key = project_key(&folder);
    let slug = format!("{}-{}", slugify(&crate::core::text::strip_verbatim(folder.clone())), &key[..6]);
    let path = crate::core::text::strip_verbatim(folder).to_string_lossy().into_owned();
    Ok((crate::core::paths::memory_dir(config)?, slug, key, path))
}
pub fn exists(config: &Path, folder: &str) -> bool {
    locate(config, folder).is_ok_and(|(root, slug, key, _)| root.join(slug).is_dir() || root.join(key).is_dir())
}
pub fn list_projects(config: &Path) -> Vec<Project> {
    let Ok(root) = crate::core::paths::memory_dir(config) else { return vec![] };
    let text = fs::read_to_string(root.join("projects.txt")).unwrap_or_default();
    text.lines()
        .filter_map(|l| {
            let mut p = l.splitn(3, " | ");
            Some(Project { slug: p.next()?.into(), path: p.next()?.into(), description: p.next().unwrap_or("").into() })
        })
        // Opening the panel registers a folder; only projects with actual notes are worth recalling.
        .filter(|p| fs::metadata(root.join(&p.slug).join("core.md")).is_ok_and(|m| m.len() > 0))
        .collect()
}
/// Another project's `core.md`; the slug must be listed in `projects.txt`.
pub fn project_core(config: &Path, slug: &str) -> Result<String> {
    ensure!(list_projects(config).iter().any(|p| p.slug == slug), "Unknown project");
    Ok(fs::read_to_string(crate::core::paths::memory_dir(config)?.join(slug).join("core.md"))?)
}

impl Store {
    pub fn open(config: &Path, folder: &str, cfg: &MemoryConfig) -> Result<Self> {
        let (root, slug, key, project) = locate(config, folder)?;
        let store = Self { dir: root.join(&slug), project, cfg: cfg.clone() };
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fs::create_dir_all(&store.dir)?;
        // A failed migration leaves the old dir in place; the next open retries.
        let migrated = migrate::run(&root, &key, &store.dir).unwrap_or(false);
        store.register(&root, &slug)?;
        if migrated || !store.dir.join("core.md").exists() {
            store.rebuild()?;
        }
        Ok(store)
    }
    fn register(&self, root: &Path, slug: &str) -> Result<()> {
        let file = root.join("projects.txt");
        let old = fs::read_to_string(&file).unwrap_or_default();
        let prefix = format!("{slug} | ");
        let mut lines: Vec<&str> = old.lines().filter(|l| !l.is_empty()).collect();
        if lines.iter().any(|l| l.starts_with(&prefix)) {
            return Ok(());
        }
        let new = format!("{prefix}{} | ", line(&self.project));
        lines.push(&new);
        Ok(fs::write(file, lines.join("\n") + "\n")?)
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        ensure!(valid_id(id) && id != "core", "Invalid session id");
        Ok(self.dir.join(format!("{id}.md")))
    }
    fn sessions(&self) -> Result<Vec<Session>> {
        let mut all = Vec::new();
        for entry in fs::read_dir(&self.dir)? {
            let path = entry?.path();
            let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            if path.extension().is_some_and(|e| e == "md") && id != "core" {
                if let Ok(s) = Session::parse(id, &fs::read_to_string(&path)?) {
                    all.push(s);
                }
            }
        }
        all.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.id.cmp(&b.id)));
        Ok(all)
    }
    fn rebuild(&self) -> Result<String> {
        let text = core_md::build(&self.sessions()?, self.cfg.max_scopes_in_prompt.clamp(1, 10) as usize);
        fs::write(self.dir.join("core.md"), &text)?;
        Ok(text)
    }
    fn save(&self, s: &Session) -> Result<String> {
        let text = s.text();
        fs::write(self.path(&s.id)?, &text)?;
        self.rebuild()?;
        Ok(text)
    }
    pub fn core(&self) -> String {
        fs::read_to_string(self.dir.join("core.md")).unwrap_or_default()
    }
    pub fn overview(&self) -> Result<Overview> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (archived, sessions) = self.sessions()?.into_iter().partition(Session::retired);
        Ok(Overview { project: self.project.clone(), core: self.rebuild()?, sessions, archived })
    }
    pub fn read(&self, id: &str) -> Result<String> {
        Ok(fs::read_to_string(self.path(id)?)?)
    }
    /// Explicit recall: open items only, capped.
    pub fn recall(&self, id: &str) -> Result<String> {
        let s = Session::parse(id, &self.read(id)?)?;
        Ok(s.loadable().chars().take(self.cfg.max_chars_per_scope as usize).collect())
    }
    fn fit(&self, s: &mut Session) -> Result<()> {
        s.updated = super::stamp();
        if s.status == "done" && s.evidence.is_empty() {
            s.status = "active".into();
            append(&mut s.open, vec!["Needs evidence to close".into()]);
        }
        while s.text().chars().count() > self.cfg.max_chars_per_scope as usize {
            ensure!(s.shrink(), "Session too long");
        }
        Ok(())
    }
    pub fn edit(&self, id: &str, text: &str) -> Result<String> {
        ensure!(text.chars().count() <= self.cfg.max_chars_per_scope as usize, "Session too long");
        let mut s = Session::parse(id, text)?;
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        ensure!(self.path(id)?.exists(), "Session not found");
        self.fit(&mut s)?;
        self.save(&s)
    }
    /// Archive = obsolete, restore = active; the file stays in place.
    pub fn archive(&self, id: &str, restore: bool) -> Result<()> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let mut s = Session::parse(id, &self.read(id)?)?;
        s.status = if restore { "active" } else { "obsolete" }.into();
        s.updated = super::stamp();
        self.save(&s).map(drop)
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        ensure!(Session::parse(id, &self.read(id)?)?.retired(), "Only archived sessions can be deleted");
        fs::remove_file(self.path(id)?)?;
        self.rebuild().map(drop)
    }
    /// Merge the AI's update into this chat's file; `files` comes from the app.
    pub fn merge(&self, u: Update, chat: &str, files: Vec<String>) -> Result<Change> {
        if !super::session::STATUSES.contains(&u.status.as_str()) {
            bail!("Invalid status");
        }
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.path(chat)?;
        let existed = path.exists();
        let mut s = if existed { Session::parse(chat, &fs::read_to_string(&path)?)? } else { Session::new(chat) };
        // A user-archived chat needs an explicit restore; a stale AI cannot revive it.
        ensure!(s.status != "obsolete", "Session archived");
        for (to, from) in [(&mut s.title, &u.title), (&mut s.goal, &u.goal)] {
            if !from.trim().is_empty() {
                *to = line(from);
            }
        }
        s.open = u.open;
        append(&mut s.open, vec![]);
        append(&mut s.decided, u.decided);
        append(&mut s.done, u.done);
        append(&mut s.evidence, u.evidence);
        append(&mut s.files, files);
        s.status = u.status;
        self.fit(&mut s)?;
        self.save(&s)?;
        let action = if s.retired() { "archived" } else if existed { "updated" } else { "created" };
        Ok(Change { scope: s.id, status: s.status, action: action.into() })
    }
}
