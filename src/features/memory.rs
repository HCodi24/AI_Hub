//! Per-folder markdown notes; no provider calls live here.
use anyhow::{bail, ensure, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime},
};

static STORE_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub max_scopes_in_prompt: u32,
    pub max_chars_per_scope: u32,
}
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_scopes_in_prompt: 3,
            max_chars_per_scope: 6000,
        }
    }
}
impl MemoryConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=10).contains(&self.max_scopes_in_prompt),
            "Số scope phải từ 1 đến 10"
        );
        ensure!(
            (1000..=20000).contains(&self.max_chars_per_scope),
            "Số ký tự phải từ 1000 đến 20000"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scope {
    pub scope: String,
    pub title: String,
    pub status: String,
    pub updated: String,
    pub chats: Vec<String>,
    pub files: Vec<String>,
    pub next: String,
    pub evidence: Vec<String>,
    pub goal: String,
    pub decisions: Vec<String>,
    pub done: Vec<String>,
    pub open: Vec<String>,
}
pub fn valid_scope(id: &str) -> bool {
    (1..=48).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
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
fn line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn scalar(s: &str) -> String {
    serde_json::to_string(&line(s)).unwrap()
}
fn list(v: &[String]) -> String {
    serde_json::to_string(v).unwrap()
}
impl Scope {
    pub fn text(&self) -> String {
        let mut s = format!(
            "---\nscope: {}\ntitle: {}\nstatus: {}\nupdated: {}\nchats: {}\nfiles: {}\nnext: {}\nevidence: {}\n---\n",
            self.scope, scalar(&self.title), self.status, self.updated, list(&self.chats), list(&self.files), scalar(&self.next), list(&self.evidence));
        for (heading, items) in [
            ("Mục tiêu", vec![self.goal.clone()]),
            ("Đã chốt", self.decisions.clone()),
            ("Đã làm", self.done.clone()),
            ("Còn mở", self.open.clone()),
        ] {
            s.push_str(&format!("\n## {heading}\n"));
            for item in items.iter().filter(|s| !s.trim().is_empty()) {
                s.push_str(&format!("- {}\n", line(item)));
            }
        }
        s
    }
    pub fn parse(text: &str) -> Result<Self> {
        let text = text.replace("\r\n", "\n");
        let rest = text
            .strip_prefix("---\n")
            .ok_or_else(|| anyhow::anyhow!("Thiếu frontmatter"))?;
        let (front, body) = rest
            .split_once("\n---\n")
            .ok_or_else(|| anyhow::anyhow!("Thiếu frontmatter"))?;
        let mut s = Self::default();
        for row in front.lines() {
            let (key, value) = row
                .split_once(':')
                .ok_or_else(|| anyhow::anyhow!("Frontmatter không hợp lệ"))?;
            let value = value.trim();
            let string =
                || serde_json::from_str::<String>(value).unwrap_or_else(|_| value.to_owned());
            match key {
                "scope" => s.scope = string(),
                "title" => s.title = string(),
                "status" => {
                    s.status = value
                        .split('#')
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_owned()
                }
                "updated" => s.updated = string(),
                "next" => s.next = string(),
                "chats" => s.chats = parse_list(value)?,
                "files" => s.files = parse_list(value)?,
                "evidence" => s.evidence = parse_list(value)?,
                _ => {}
            }
        }
        ensure!(valid_scope(&s.scope), "Scope không hợp lệ");
        ensure!(
            ["active", "blocked", "done", "obsolete"].contains(&s.status.as_str()),
            "Trạng thái không hợp lệ"
        );
        let mut section = "";
        for row in body.lines() {
            if let Some(h) = row.strip_prefix("## ") {
                section = h;
            }
            if let Some(item) = row.strip_prefix("- ") {
                match section {
                    "Mục tiêu" => {
                        if !s.goal.is_empty() {
                            s.goal.push(' ');
                        }
                        s.goal.push_str(item);
                    }
                    "Đã chốt" => s.decisions.push(item.into()),
                    "Đã làm" => s.done.push(item.into()),
                    "Còn mở" => s.open.push(item.into()),
                    _ => {}
                }
            }
        }
        Ok(s)
    }
}
fn parse_list(s: &str) -> Result<Vec<String>> {
    if let Ok(v) = serde_json::from_str(s) {
        return Ok(v);
    }
    let inner = s
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| anyhow::anyhow!("Danh sách không hợp lệ"))?;
    Ok(inner
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect())
}
fn append(to: &mut Vec<String>, from: Vec<String>) {
    let values = std::mem::take(to).into_iter().chain(from);
    for value in values {
        let value = line(&value);
        if !value.is_empty() && !to.contains(&value) {
            to.push(value);
        }
    }
    if to.len() > 30 {
        to.drain(..to.len() - 30);
    }
}
fn stamp() -> String {
    // Gregorian civil date from Unix days, independent of local timezone.
    let secs = crate::mutis::now();
    let z = (secs / 86400) as i64 + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        secs % 86400 / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}
pub fn strip_blocks(text: &str) -> (String, Option<String>) {
    let mut clean = String::new();
    let mut rest = text;
    let mut last = None;
    while let Some(start) = rest.find("<memory>") {
        clean.push_str(&rest[..start]);
        rest = &rest[start + 8..];
        let Some(end) = rest.find("</memory>") else {
            return (clean, last);
        };
        last = Some(rest[..end].to_owned());
        rest = &rest[end + 9..];
    }
    clean.push_str(rest);
    (clean, last)
}
pub const INSTRUCTION: &str = concat!(
    "\nCuối câu trả lời, ghi đúng MỘT khối <memory>{\"scope\":\"id\",\"title\":\"…\",\"status\":\"active\",",
    "\"goal\":\"…\",\"decisions\":[\"…\"],\"done\":[\"…\"],\"open\":[\"…\"],\"next\":\"…\",\"files\":[],\"evidence\":[]}</memory>",
    " hoặc <memory>none</memory> nếu không đổi. Tóm tắt công việc; dùng lại scope trong index khi cùng chủ đề; id [a-z0-9-]{1,48}.",
    " status: active|blocked|done|obsolete. Chỉ done khi có evidence (commit hash hoặc user-confirmed nếu người dùng nói đã xong).",
    " obsolete khi người dùng bỏ hoặc đổi hướng. Không tự bịa bằng chứng."
);

pub struct Store {
    root: PathBuf,
    pub project: String,
}
#[derive(Serialize)]
pub struct Overview {
    pub project: String,
    pub index: String,
    pub scopes: Vec<Scope>,
    pub archived: Vec<Scope>,
}
#[derive(Serialize)]
pub struct Change {
    pub scope: String,
    pub status: String,
    pub action: String,
}
impl Store {
    pub fn open(config: &Path, folder: &str) -> Result<Self> {
        let folder = fs::canonicalize(folder)?;
        ensure!(folder.is_dir(), "Thư mục không tồn tại");
        let root = crate::core::paths::memory_dir(config)?.join(project_key(&folder));
        fs::create_dir_all(root.join("archive"))?;
        let project = crate::core::text::strip_verbatim(folder)
            .to_string_lossy()
            .into_owned();
        let store = Self { root, project };
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        fs::write(store.root.join("project.txt"), &store.project)?;
        store.purge(SystemTime::now())?;
        store.index()?;
        Ok(store)
    }
    fn path(&self, id: &str, archived: bool) -> Result<PathBuf> {
        ensure!(valid_scope(id), "Scope không hợp lệ");
        Ok(if archived {
            self.root.join("archive")
        } else {
            self.root.clone()
        }
        .join(format!("{id}.md")))
    }
    fn scopes(&self, archived: bool) -> Result<Vec<Scope>> {
        let dir = if archived {
            self.root.join("archive")
        } else {
            self.root.clone()
        };
        let mut scopes = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "md") {
                if let Ok(s) = Scope::parse(&fs::read_to_string(path)?) {
                    scopes.push(s);
                }
            }
        }
        scopes.sort_by(|a, b| b.updated.cmp(&a.updated).then(a.scope.cmp(&b.scope)));
        Ok(scopes)
    }
    fn index(&self) -> Result<String> {
        let text = self
            .scopes(false)?
            .iter()
            .map(|s| {
                format!(
                    "- [{}] {} — {} · next: {} · {}\n",
                    s.status,
                    s.scope,
                    line(&s.title),
                    line(&s.next),
                    s.updated
                )
            })
            .collect::<String>();
        fs::write(self.root.join("index.md"), &text)?;
        Ok(text)
    }
    fn purge(&self, now: SystemTime) -> Result<()> {
        for entry in fs::read_dir(self.root.join("archive"))? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "md")
                && now
                    .duration_since(entry.metadata()?.modified()?)
                    .unwrap_or_default()
                    > Duration::from_secs(14 * 86400)
            {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }
    pub fn overview(&self) -> Result<Overview> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        Ok(Overview {
            project: self.project.clone(),
            index: self.index()?,
            scopes: self.scopes(false)?,
            archived: self.scopes(true)?,
        })
    }
    pub fn read(&self, id: &str) -> Result<String> {
        let path = self.path(id, false)?;
        Ok(fs::read_to_string(if path.exists() {
            path
        } else {
            self.path(id, true)?
        })?)
    }
    pub fn edit(&self, id: &str, text: &str, config: &MemoryConfig) -> Result<String> {
        let mut scope = Scope::parse(text)?;
        ensure!(scope.scope == id, "Scope frontmatter không khớp");
        ensure!(
            text.chars().count() <= config.max_chars_per_scope as usize,
            "Scope quá dài"
        );
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        scope.updated = stamp();
        scope.evidence.retain(|value| !value.trim().is_empty());
        if scope.status == "done" && scope.evidence.is_empty() {
            scope.status = "active".into();
            append(&mut scope.open, vec!["Cần bằng chứng để đóng scope".into()]);
        }
        for items in [
            &mut scope.decisions,
            &mut scope.done,
            &mut scope.open,
            &mut scope.files,
            &mut scope.chats,
            &mut scope.evidence,
        ] {
            ensure!(items.len() <= 30, "Danh sách tối đa 30 mục");
        }
        let archived =
            self.path(id, true)?.exists() || matches!(scope.status.as_str(), "done" | "obsolete");
        let text = scope.text();
        ensure!(
            text.chars().count() <= config.max_chars_per_scope as usize,
            "Scope quá dài"
        );
        fs::write(self.path(id, archived)?, &text)?;
        if archived && self.path(id, false)?.exists() {
            fs::remove_file(self.path(id, false)?)?;
        }
        self.index()?;
        Ok(text)
    }
    pub fn archive(&self, id: &str, restore: bool) -> Result<()> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let source = self.path(id, restore)?;
        let dest = self.path(id, !restore)?;
        ensure!(!dest.exists(), "Scope đích đã tồn tại");
        let mut s = Scope::parse(&fs::read_to_string(&source)?)?;
        s.status = if restore { "active" } else { "obsolete" }.into();
        s.updated = stamp();
        fs::write(&dest, s.text())?;
        fs::remove_file(source)?;
        self.index()?;
        Ok(())
    }
    pub fn delete(&self, id: &str) -> Result<()> {
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        ensure!(!self.path(id, false)?.exists(), "Chỉ xoá scope đã lưu trữ");
        fs::remove_file(self.path(id, true)?)?;
        self.index()?;
        Ok(())
    }
    pub fn merge(&self, mut update: Scope, chat: &str, config: &MemoryConfig) -> Result<Change> {
        ensure!(valid_scope(&update.scope), "Scope không hợp lệ");
        ensure!(
            ["active", "blocked", "done", "obsolete"].contains(&update.status.as_str()),
            "Trạng thái không hợp lệ"
        );
        let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let path = self.path(&update.scope, false)?;
        // Retired scopes require an explicit restore; a stale AI cannot revive them.
        ensure!(
            !self.path(&update.scope, true)?.exists(),
            "Scope đã lưu trữ"
        );
        let existed = path.exists();
        let mut s = if existed {
            Scope::parse(&fs::read_to_string(&path)?)?
        } else {
            Scope {
                scope: update.scope.clone(),
                ..Default::default()
            }
        };
        for (to, from) in [
            (&mut s.title, &update.title),
            (&mut s.goal, &update.goal),
            (&mut s.next, &update.next),
        ] {
            if !from.trim().is_empty() {
                *to = line(from);
            }
        }
        if !update.files.is_empty() {
            s.files = std::mem::take(&mut update.files);
        }
        s.open = update.open;
        append(&mut s.open, vec![]);
        append(&mut s.decisions, update.decisions);
        append(&mut s.done, update.done);
        append(&mut s.evidence, update.evidence);
        append(&mut s.chats, vec![chat.into()]);
        append(&mut s.files, vec![]);
        s.status = update.status;
        s.updated = stamp();
        if s.status == "done" && s.evidence.is_empty() {
            s.status = "active".into();
            append(&mut s.open, vec!["Cần bằng chứng để đóng scope".into()]);
        }
        while s.text().chars().count() > config.max_chars_per_scope as usize {
            let list = if !s.done.is_empty() {
                &mut s.done
            } else if !(s.open.is_empty()
                || s.open.len() == 1 && s.open[0] == "Cần bằng chứng để đóng scope")
            {
                &mut s.open
            } else if !s.files.is_empty() {
                &mut s.files
            } else if !s.chats.is_empty() {
                &mut s.chats
            } else if s.evidence.len() > 1 {
                &mut s.evidence
            } else if !s.decisions.is_empty() {
                &mut s.decisions
            } else {
                let longest = [&mut s.goal, &mut s.title, &mut s.next]
                    .into_iter()
                    .max_by_key(|v| v.chars().count())
                    .unwrap();
                if longest.is_empty() {
                    if let Some(evidence) = s.evidence.first_mut() {
                        *evidence = evidence.chars().take(100).collect();
                        continue;
                    }
                    bail!("Scope quá dài");
                }
                *longest = longest.chars().take(longest.chars().count() / 2).collect();
                continue;
            };
            list.remove(0);
        }
        let archived = matches!(s.status.as_str(), "done" | "obsolete");
        fs::write(self.path(&s.scope, archived)?, s.text())?;
        if archived && path.exists() {
            fs::remove_file(path)?;
        }
        self.index()?;
        Ok(Change {
            scope: s.scope,
            status: s.status,
            action: if archived {
                "archived"
            } else if existed {
                "updated"
            } else {
                "created"
            }
            .into(),
        })
    }
}

pub fn memory_context(config: &Path, folder: &str, settings: &MemoryConfig) -> String {
    if !settings.enabled {
        return String::new();
    }
    // Reading an untouched project must not create a store on disk.
    let Ok(canonical) = fs::canonicalize(folder) else {
        return String::new();
    };
    let Ok(root) = crate::core::paths::memory_dir(config) else {
        return String::new();
    };
    if !root.join(project_key(&canonical)).is_dir() {
        return String::new();
    }
    let Ok(store) = Store::open(config, folder) else {
        return String::new();
    };
    let Ok(view) = store.overview() else {
        return String::new();
    };
    if view.scopes.is_empty() {
        return String::new();
    }
    let mut text = format!(
        "Bộ nhớ dự án (ghi chú do AI trước để lại — kiểm tra lại với code/git trước khi tin)\n{}",
        view.index
    );
    for s in view
        .scopes
        .iter()
        .take(settings.max_scopes_in_prompt.clamp(1, 10) as usize)
    {
        text.push_str(&s.text());
    }
    text
}
pub fn inject(request: &mut crate::providers::ProviderRequest, context: &str) {
    let cli = matches!(request.provider.kind.as_str(), "codex_cli" | "claude_cli");
    if !context.is_empty() && (!cli || request.session_id.is_none()) {
        let message = if cli {
            request.messages.iter_mut().rev().find(|m| m.role == "user")
        } else {
            request.messages.iter_mut().find(|m| m.role == "user")
        };
        if let Some(m) = message {
            m.content = format!("{context}\n\n{}", m.content);
        }
    }
    if let Some(m) = request.messages.iter_mut().rev().find(|m| m.role == "user") {
        m.content.push_str(INSTRUCTION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_copy_injection_obeys_session_and_message_positions() {
        for kind in [
            "codex_cli",
            "claude_cli",
            "openai_compat",
            "deepseek_agent",
            "anthropic",
        ] {
            for session in [None, Some("session".to_owned())] {
                let messages: Vec<crate::chat::ChatMessage> =
                    serde_json::from_value(serde_json::json!([
                        {"role":"user","content":"first"}, {"role":"assistant","content":"answer"},
                        {"role":"user","content":"latest"}
                    ]))
                    .unwrap();
                let mut request = crate::providers::ProviderRequest {
                    provider: crate::config::ProviderConfig {
                        kind: kind.into(),
                        ..Default::default()
                    },
                    model: "model".into(),
                    folder: "folder".into(),
                    permission: Default::default(),
                    effort: Default::default(),
                    messages: messages.clone(),
                    session_id: session.clone(),
                    mutis: Default::default(),
                };
                inject(&mut request, "MEMORY_CONTEXT");
                let cli = matches!(kind, "codex_cli" | "claude_cli");
                assert_eq!(request.messages[0].content.contains("MEMORY_CONTEXT"), !cli);
                assert_eq!(
                    request.messages[2].content.contains("MEMORY_CONTEXT"),
                    cli && session.is_none()
                );
                assert!(request.messages[2].content.ends_with(INSTRUCTION));
                assert_eq!(request.messages[1].content, "answer");
                assert_eq!(messages[0].content, "first");
                assert_eq!(messages[2].content, "latest");
            }
        }
    }
    struct Fixture {
        root: PathBuf,
        config: PathBuf,
        store: Store,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("ai-hub-memory-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).unwrap();
            let config = root.join("config.json");
            let store = Store::open(&config, root.to_str().unwrap()).unwrap();
            Self {
                root,
                config,
                store,
            }
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn update(id: &str, status: &str) -> Scope {
        Scope {
            scope: id.into(),
            status: status.into(),
            title: "Bộ nhớ tiếng Việt".into(),
            goal: "Giữ ngữ cảnh dự án".into(),
            next: "Kiểm tra code".into(),
            ..Default::default()
        }
    }
    #[test]
    fn vietnamese_frontmatter_roundtrip() {
        let mut s = update("bo-nho", "blocked");
        s.title = "Tiêu đề: dấu \"nháy\" và đường dẫn C:\\TOOL".into();
        s.decisions = vec!["Giữ nguyên tiếng Việt".into()];
        s.files = vec!["src/bộ nhớ.rs".into()];
        let parsed = Scope::parse(&s.text()).unwrap();
        assert_eq!(parsed.title, s.title);
        assert_eq!(parsed.goal, s.goal);
        assert_eq!(parsed.decisions, s.decisions);
        assert_eq!(parsed.files, s.files);
        assert_eq!(parsed.text(), s.text());
        assert_eq!(stamp().len(), 20);
    }
    #[test]
    fn parses_unquoted_lists_and_rejects_bad_paths() {
        let text = update("test", "active")
            .text()
            .replace("files: []", "files: [src/api/debate.rs]");
        assert_eq!(
            Scope::parse(&text).unwrap().files,
            vec!["src/api/debate.rs"]
        );
        for id in [
            "",
            "../scope",
            "a/b",
            "a\\b",
            "A",
            "a.b",
            "bộ-nhớ",
            &"x".repeat(49),
        ] {
            assert!(!valid_scope(id));
            assert!(Fixture::new().store.path(id, false).is_err());
        }
        assert!(valid_scope(&"a".repeat(48)));
    }
    #[test]
    fn extracts_last_and_strips_all_complete_and_incomplete_blocks() {
        let (text, last) = strip_blocks(
            "a<memory>none</memory>b<memory>{\"scope\":\"x\"}</memory>c<memory>unfinished",
        );
        assert_eq!(text, "abc");
        assert_eq!(last.as_deref(), Some("{\"scope\":\"x\"}"));
        assert_eq!(strip_blocks("plain"), ("plain".into(), None));
        assert!(INSTRUCTION.chars().count() <= 700);
    }
    #[test]
    fn merge_preserves_decisions_done_and_deduplicates() {
        let f = Fixture::new();
        let config = MemoryConfig::default();
        let mut a = update("topic", "active");
        a.decisions = vec!["A".into()];
        a.done = vec!["D".into()];
        a.open = vec!["old".into()];
        assert_eq!(
            f.store.merge(a, "chat-1", &config).unwrap().action,
            "created"
        );
        let mut b = update("topic", "blocked");
        b.decisions = vec!["A".into(), "B".into()];
        b.open = vec!["new".into()];
        assert_eq!(
            f.store.merge(b.clone(), "chat-2", &config).unwrap().action,
            "updated"
        );
        f.store.merge(b, "chat-2", &config).unwrap();
        let s = Scope::parse(&f.store.read("topic").unwrap()).unwrap();
        assert_eq!(s.decisions, vec!["A", "B"]);
        assert_eq!(s.done, vec!["D"]);
        assert_eq!(s.open, vec!["new"]);
        assert_eq!(s.chats, vec!["chat-1", "chat-2"]);
    }
    #[test]
    fn caps_drop_oldest_work_before_decisions_and_bound_whole_file() {
        let f = Fixture::new();
        let config = MemoryConfig {
            max_chars_per_scope: 1000,
            ..Default::default()
        };
        let mut s = update("topic", "active");
        s.decisions = vec!["Giữ quyết định này".into()];
        s.done = (0..40)
            .map(|i| format!("{i}: {}", "x".repeat(90)))
            .collect();
        s.open = (0..40).map(|i| i.to_string()).collect();
        f.store.merge(s, "chat", &config).unwrap();
        let text = f.store.read("topic").unwrap();
        let s = Scope::parse(&text).unwrap();
        assert!(text.chars().count() <= 1000);
        assert!(s.open.len() <= 30);
        assert!(s.done.len() <= 30);
        assert_eq!(s.decisions, vec!["Giữ quyết định này"]);
        assert!(!s.open.contains(&"0".into()));
    }
    #[test]
    fn done_needs_evidence_and_archives_then_restores() {
        let f = Fixture::new();
        let config = MemoryConfig::default();
        let change = f
            .store
            .merge(update("topic", "done"), "chat", &config)
            .unwrap();
        assert_eq!(change.status, "active");
        assert!(f
            .store
            .read("topic")
            .unwrap()
            .contains("Cần bằng chứng để đóng scope"));
        let mut s = update("topic", "done");
        s.evidence = vec!["user-confirmed".into()];
        assert_eq!(
            f.store.merge(s, "chat", &config).unwrap().action,
            "archived"
        );
        assert!(f.store.overview().unwrap().scopes.is_empty());
        assert!(f
            .store
            .merge(update("topic", "active"), "chat", &config)
            .is_err());
        f.store.archive("topic", true).unwrap();
        assert_eq!(f.store.overview().unwrap().scopes[0].status, "active");
        assert!(f.store.delete("topic").is_err());
    }
    #[test]
    fn obsolete_archive_and_manual_edit_index() {
        let f = Fixture::new();
        let config = MemoryConfig::default();
        f.store
            .merge(update("topic", "obsolete"), "chat", &config)
            .unwrap();
        assert_eq!(f.store.overview().unwrap().archived.len(), 1);
        f.store.archive("topic", true).unwrap();
        let mut s = Scope::parse(&f.store.read("topic").unwrap()).unwrap();
        s.next = "Bước tiếp theo mới".into();
        f.store.edit("topic", &s.text(), &config).unwrap();
        assert!(f
            .store
            .overview()
            .unwrap()
            .index
            .contains("Bước tiếp theo mới"));
        assert!(f.store.edit("other", &s.text(), &config).is_err());
        f.store.archive("topic", false).unwrap();
        f.store.delete("topic").unwrap();
        assert!(f.store.overview().unwrap().archived.is_empty());
    }
    #[test]
    fn key_strips_verbatim_and_normalizes_case() {
        assert_eq!(
            project_key(Path::new(r"\\?\C:\TOOL\Project")),
            project_key(Path::new(r"c:\tool\project"))
        );
        assert_eq!(project_key(Path::new("abc")), "ba7816bf8f01cfea");
    }
    #[test]
    fn context_orders_limits_and_disabled_skips() {
        let f = Fixture::new();
        let mut config = MemoryConfig {
            max_scopes_in_prompt: 1,
            ..Default::default()
        };
        for (id, date, goal) in [
            ("older", "2026-01-01T00:00:00Z", "OLD_FULL_TEXT"),
            ("newer", "2026-11-10T00:00:00Z", "NEW_FULL_TEXT"),
        ] {
            let mut s = update(id, "active");
            s.updated = date.into();
            s.goal = goal.into();
            fs::write(f.store.path(id, false).unwrap(), s.text()).unwrap();
        }
        let text = memory_context(&f.config, f.root.to_str().unwrap(), &config);
        assert!(text.contains("NEW_FULL_TEXT"));
        assert!(!text.contains("OLD_FULL_TEXT"));
        assert!(text.find("newer").unwrap() < text.find("older").unwrap());
        config.enabled = false;
        assert!(memory_context(&f.config, f.root.to_str().unwrap(), &config).is_empty());
    }
    #[test]
    fn opening_purges_only_archives_older_than_fourteen_days() {
        let f = Fixture::new();
        let config = MemoryConfig::default();
        for id in ["old", "recent"] {
            f.store
                .merge(update(id, "obsolete"), "chat", &config)
                .unwrap();
        }
        let old = f.store.path("old", true).unwrap();
        fs::File::options()
            .write(true)
            .open(&old)
            .unwrap()
            .set_times(
                fs::FileTimes::new()
                    .set_modified(SystemTime::now() - Duration::from_secs(15 * 86400)),
            )
            .unwrap();
        let store = Store::open(&f.config, f.root.to_str().unwrap()).unwrap();
        assert!(!old.exists());
        assert_eq!(store.overview().unwrap().archived[0].scope, "recent");
    }
}
