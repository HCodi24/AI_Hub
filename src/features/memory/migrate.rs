//! Old layout (`<hash>/<scope>.md` + `archive/`) → `legacy-<scope>.md` sessions. Old dir is renamed, never deleted.
use super::session::{line, Session, STATUSES};
use anyhow::Result;
use std::{fs, path::Path};

fn unquote(v: &str) -> String {
    serde_json::from_str::<String>(v).unwrap_or_else(|_| v.to_owned())
}
fn list(v: &str) -> Vec<String> {
    serde_json::from_str(v).unwrap_or_default()
}

fn convert(text: &str) -> Option<Session> {
    let text = text.replace("\r\n", "\n");
    let (front, body) = text.strip_prefix("---\n")?.split_once("\n---\n")?;
    let mut s = Session::default();
    let mut scope = String::new();
    for row in front.lines() {
        let Some((key, v)) = row.split_once(':') else { continue };
        let v = v.trim();
        match key {
            "scope" => scope = unquote(v),
            "title" => s.title = unquote(v),
            "status" => s.status = v.split('#').next().unwrap_or_default().trim().into(),
            "updated" => s.updated = unquote(v),
            "files" => s.files = list(v),
            "evidence" => s.evidence = list(v),
            _ => {}
        }
    }
    if scope.is_empty() {
        return None;
    }
    if !STATUSES.contains(&s.status.as_str()) {
        s.status = "active".into();
    }
    s.id = format!("legacy-{scope}");
    let mut section = "";
    let mut goal = Vec::new();
    for row in body.lines() {
        if let Some(h) = row.strip_prefix("## ") {
            section = h;
        } else if let Some(item) = row.strip_prefix("- ") {
            let item = line(item);
            match section {
                "Mục tiêu" => goal.push(item),
                "Đã chốt" => s.decided.push(item),
                "Đã làm" => s.done.push(item),
                "Còn mở" => s.open.push(item),
                _ => {}
            }
        }
    }
    s.goal = goal.join(" ");
    Some(s)
}

/// True when an old dir was found. Files are written first and the rename last, so a crash just reruns.
pub fn run(root: &Path, key: &str, new_dir: &Path) -> Result<bool> {
    let old = root.join(key);
    if !old.is_dir() {
        return Ok(false);
    }
    for dir in [old.clone(), old.join("archive")] {
        let Ok(entries) = fs::read_dir(dir) else { continue };
        for path in entries.flatten().map(|e| e.path()) {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
            if path.extension().is_none_or(|e| e != "md") || name == "index.md" {
                continue;
            }
            let Some(s) = convert(&fs::read_to_string(&path)?) else { continue };
            let target = new_dir.join(format!("{}.md", s.id));
            if !target.exists() {
                fs::write(target, s.text())?;
            }
        }
    }
    let bak = root.join(format!("{key}.bak"));
    if !bak.exists() {
        fs::rename(&old, bak)?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_old_scope_and_keeps_backup() {
        let root = std::env::temp_dir().join(format!("mig-{}", uuid::Uuid::new_v4()));
        let (old, new) = (root.join("abcd1234abcd1234"), root.join("proj-abcd12"));
        fs::create_dir_all(old.join("archive")).unwrap();
        fs::create_dir_all(&new).unwrap();
        fs::write(old.join("index.md"), "ignored").unwrap();
        let scope = "---\nscope: bo-nho\ntitle: \"Bộ nhớ\"\nstatus: active\nupdated: 2026-01-01T00:00:00Z\nchats: []\nfiles: [\"a.rs\"]\nnext: \"x\"\nevidence: []\n---\n\n## Mục tiêu\n- Giữ ngữ cảnh\n\n## Đã chốt\n- Chọn A\n\n## Đã làm\n- Xong B\n\n## Còn mở\n- Việc C\n";
        fs::write(old.join("bo-nho.md"), scope).unwrap();
        fs::write(old.join("archive").join("cu.md"), scope.replace("bo-nho", "cu").replace("active", "obsolete")).unwrap();
        assert!(run(&root, "abcd1234abcd1234", &new).unwrap());
        let s = Session::parse("legacy-bo-nho", &fs::read_to_string(new.join("legacy-bo-nho.md")).unwrap()).unwrap();
        assert_eq!((s.title.as_str(), s.goal.as_str()), ("Bộ nhớ", "Giữ ngữ cảnh"));
        assert_eq!((&s.decided[0][..], &s.done[0][..], &s.open[0][..], &s.files[0][..]), ("Chọn A", "Xong B", "Việc C", "a.rs"));
        assert!(new.join("legacy-cu.md").exists() && root.join("abcd1234abcd1234.bak/bo-nho.md").exists());
        assert!(!old.exists() && !run(&root, "abcd1234abcd1234", &new).unwrap());
        let _ = fs::remove_dir_all(root);
    }
}
