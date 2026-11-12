use super::*;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    config: PathBuf,
    cfg: MemoryConfig,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("ai-hub-memory-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        Self { config: root.join("config.json"), root, cfg: MemoryConfig::default() }
    }
    fn store(&self) -> Store {
        Store::open(&self.config, self.root.to_str().unwrap(), &self.cfg).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn update(status: &str) -> Update {
    Update { title: "Login fix".into(), status: status.into(), goal: "Fix login".into(), ..Default::default() }
}

#[test]
fn merge_updates_chat_file_keeps_app_files_and_rebuilds_core() {
    let f = Fixture::new();
    let store = f.store();
    let mut a = update("active");
    a.open = vec!["write test".into()];
    a.done = vec!["found bug".into()];
    assert_eq!(store.merge(a, "chat-1", vec!["src/a.rs".into()]).unwrap().action, "created");
    let mut b = update("blocked");
    b.open = vec!["ask user".into()];
    assert_eq!(store.merge(b, "chat-1", vec!["src/b.rs".into(), "src/a.rs".into()]).unwrap().action, "updated");
    let s = Session::parse("chat-1", &store.read("chat-1").unwrap()).unwrap();
    assert_eq!(s.files, vec!["src/a.rs", "src/b.rs"]);
    assert_eq!((s.open, s.done), (vec!["ask user".to_string()], vec!["found bug".to_string()]));
    assert!(store.core().contains("[chat-1] ask user") && !store.core().contains("write test"));
    assert!(!store.recall("chat-1").unwrap().contains("found bug"));
    assert_eq!(list_projects(&f.config).len(), 1);
    assert!(project_core(&f.config, &list_projects(&f.config)[0].slug).unwrap().contains("Login fix"));
}

#[test]
fn done_needs_evidence_and_archive_blocks_stale_revival() {
    let f = Fixture::new();
    let store = f.store();
    assert_eq!(store.merge(update("done"), "c", vec![]).unwrap().status, "active");
    let mut d = update("done");
    d.evidence = vec!["user-confirmed".into()];
    assert_eq!(store.merge(d, "c", vec![]).unwrap().action, "archived");
    assert!(store.delete("c").is_ok());
    store.merge(update("active"), "c", vec![]).unwrap();
    store.archive("c", false).unwrap();
    assert!(store.merge(update("active"), "c", vec![]).is_err());
    assert_eq!(store.overview().unwrap().archived.len(), 1);
    store.archive("c", true).unwrap();
    assert_eq!(store.overview().unwrap().sessions.len(), 1);
    assert!(store.delete("c").is_err());
}

#[test]
fn cap_bounds_session_and_ids_are_validated() {
    let mut f = Fixture::new();
    f.cfg.max_chars_per_scope = 1000;
    let store = f.store();
    let mut u = update("active");
    u.done = (0..30).map(|i| format!("{i} {}", "x".repeat(90))).collect();
    store.merge(u, "c", vec![]).unwrap();
    assert!(store.read("c").unwrap().chars().count() <= 1000);
    for id in ["", "../x", "a/b", "core", &"x".repeat(65)] {
        assert!(store.read(id).is_err());
    }
}

#[test]
fn open_migrates_old_layout_once() {
    let f = Fixture::new();
    let key = store::project_key(&fs::canonicalize(&f.root).unwrap());
    let old = f.root.join("memory").join(&key);
    fs::create_dir_all(&old).unwrap();
    fs::write(old.join("a.md"), "---\nscope: a\ntitle: \"T\"\nstatus: active\nupdated: 2026-01-01T00:00:00Z\n---\n\n## Còn mở\n- việc\n").unwrap();
    let text = memory_context(&f.config, f.root.to_str().unwrap(), &f.cfg);
    assert!(text.contains("[legacy-a] việc"));
    assert!(f.root.join("memory").join(format!("{key}.bak")).is_dir());
    assert!(memory_context(&f.config, f.root.to_str().unwrap(), &f.cfg).contains("legacy-a"));
}

#[test]
fn context_respects_disabled_and_untouched_projects() {
    let f = Fixture::new();
    let folder = f.root.to_str().unwrap();
    assert!(memory_context(&f.config, folder, &f.cfg).is_empty());
    assert!(!f.root.join("memory").exists());
    f.store().merge(update("active"), "c", vec![]).unwrap();
    assert!(memory_context(&f.config, folder, &f.cfg).contains("Login fix"));
    assert!(memory_context(&f.config, folder, &MemoryConfig { enabled: false, ..Default::default() }).is_empty());
}

#[test]
fn strip_blocks_and_instruction_size() {
    let (text, last) = strip_blocks("a<memory>none</memory>b<memory>{\"x\":1}</memory>c<memory>unfinished");
    assert_eq!((text.as_str(), last.as_deref()), ("abc", Some("{\"x\":1}")));
    assert_eq!(strip_blocks("plain"), ("plain".into(), None));
    assert!(INSTRUCTION.chars().count() <= 700 && stamp().len() == 20);
}
