use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FILES: usize = 2000;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", ".next", "dist"];

#[derive(Default)]
pub(super) struct Snapshot {
    files: BTreeMap<String, Option<Vec<u8>>>,
    ignored: BTreeSet<String>,
    directories: BTreeSet<String>,
}

impl Snapshot {
    pub(super) fn take(folder: &str, files: &[String]) -> Self {
        let root = Path::new(folder);
        let mut snapshot = Self::default();
        for path in files.iter().filter_map(|path| relative_path(path)) {
            if snapshot.files.len() + snapshot.ignored.len() >= MAX_FILES {
                break;
            }
            match metadata(root, &path) {
                Ok(info) if info.is_dir() => {
                    snapshot.directories.insert(path.clone());
                    walk(root, &path, &mut |file| {
                        snapshot.capture(root, file);
                        snapshot.files.len() + snapshot.ignored.len() < MAX_FILES
                    });
                }
                Ok(info) if info.is_file() => snapshot.capture(root, path),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    snapshot.files.insert(path, None);
                }
                _ => {}
            }
        }
        snapshot
    }

    fn capture(&mut self, root: &Path, path: String) {
        if self.files.contains_key(&path) || self.ignored.contains(&path) {
            return;
        }
        match contents(root, &path) {
            Ok(bytes) => {
                self.files.insert(path, bytes);
            }
            Err(_) => {
                self.ignored.insert(path);
            }
        }
    }

    pub(super) fn diff(&self, folder: &str) -> String {
        let root = Path::new(folder);
        let mut diff = String::new();
        for (path, old) in &self.files {
            if let Ok(new) = contents(root, path) {
                if old != &new {
                    section(&mut diff, path, old.as_deref(), new.as_deref());
                }
            }
        }
        let mut created = BTreeSet::new();
        let remaining = MAX_FILES - self.files.len() - self.ignored.len();
        for dir in &self.directories {
            if created.len() == remaining {
                break;
            }
            walk(root, dir, &mut |path| {
                if !self.files.contains_key(&path) && !self.ignored.contains(&path) {
                    created.insert(path);
                }
                created.len() < remaining
            });
        }
        for path in created {
            if let Ok(Some(bytes)) = contents(root, &path) {
                section(&mut diff, &path, None, Some(&bytes));
            }
        }
        diff
    }
}

fn relative_path(path: &str) -> Option<String> {
    let normalized = path.replace('\\', "/");
    if normalized.is_empty() || normalized.contains([':', '\r', '\n', '\0']) {
        return None;
    }
    let mut parts = Vec::new();
    for component in Path::new(&normalized).components() {
        match component {
            Component::Normal(name) => parts.push(name.to_str()?),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(if parts.is_empty() { ".".into() } else { parts.join("/") })
}

// Check each component so a directory symlink cannot lead outside the workspace.
fn metadata(root: &Path, path: &str) -> io::Result<Metadata> {
    let mut full_path = root.to_path_buf();
    for part in Path::new(path).components() {
        full_path.push(part);
        let info = fs::symlink_metadata(&full_path)?;
        if info.is_symlink() {
            return Err(io::Error::other("symlink"));
        }
    }
    fs::symlink_metadata(full_path)
}

fn contents(root: &Path, path: &str) -> io::Result<Option<Vec<u8>>> {
    let info = match metadata(root, path) {
        Ok(info) if info.is_file() => info,
        Ok(_) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if info.len() > MAX_BYTES {
        return Err(io::Error::other("file too large"));
    }
    let mut bytes = Vec::new();
    File::open(root.join(path))?.take(MAX_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(io::Error::other("file too large"));
    }
    Ok(Some(bytes))
}

fn walk(root: &Path, dir: &str, visit: &mut impl FnMut(String) -> bool) -> bool {
    if dir.split('/').any(|part| SKIP_DIRS.contains(&part))
        || !metadata(root, dir).is_ok_and(|info| info.is_dir())
    {
        return true;
    }
    let Ok(entries) = fs::read_dir(root.join(dir)) else { return true };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if name.contains(['\\', ':', '\r', '\n']) {
            continue;
        }
        let path = if dir == "." { name.to_owned() } else { format!("{dir}/{name}") };
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if !walk(root, &path, visit) {
                return false;
            }
        } else if kind.is_file() && !visit(path) {
            return false;
        }
    }
    true
}

fn section(diff: &mut String, path: &str, old: Option<&[u8]>, new: Option<&[u8]>) {
    use std::fmt::Write;

    let (old_bytes, new_bytes) = (old.unwrap_or_default(), new.unwrap_or_default());
    let (Ok(old_text), Ok(new_text)) = (std::str::from_utf8(old_bytes), std::str::from_utf8(new_bytes)) else {
        let _ = writeln!(diff, "Binary file {path} changed");
        return;
    };
    if old_bytes.contains(&0) || new_bytes.contains(&0) {
        let _ = writeln!(diff, "Binary file {path} changed");
        return;
    }
    let old_header = if old.is_some() { format!("a/{path}") } else { "/dev/null".into() };
    let new_header = if new.is_some() { format!("b/{path}") } else { "/dev/null".into() };
    let _ = writeln!(diff, "diff --git a/{path} b/{path}");
    if old.is_none() {
        diff.push_str("new file mode 100644\n");
    } else if new.is_none() {
        diff.push_str("deleted file mode 100644\n");
    }
    let _ = writeln!(diff, "--- {old_header}\n+++ {new_header}");
    if let Some(hunks) = git_hunks(old_bytes, new_bytes) {
        diff.push_str(&hunks);
    } else if !old_text.is_empty() || !new_text.is_empty() {
        let old_count = old_text.lines().count();
        let new_count = new_text.lines().count();
        let _ = writeln!(diff, "@@ -{},{old_count} +{},{new_count} @@", usize::from(old_count > 0), usize::from(new_count > 0));
        for (prefix, text) in [('-', old_text), ('+', new_text)] {
            for line in text.split_inclusive('\n') {
                diff.push(prefix);
                diff.push_str(line);
                if !line.ends_with('\n') {
                    diff.push_str("\n\\ No newline at end of file\n");
                }
            }
        }
    }
}

fn git_hunks(old: &[u8], new: &[u8]) -> Option<String> {
    if old == new {
        return None;
    }
    let temp = TempDiff::create().ok()?;
    fs::write(temp.0.join("old"), old).ok()?;
    fs::write(temp.0.join("new"), new).ok()?;
    let mut command = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let result = command
        .stdin(std::process::Stdio::null())
        .current_dir(&temp.0)
        .args(["diff", "--no-index", "--no-color", "--no-ext-diff", "--no-textconv", "-U3", "--", "old", "new"])
        .output().ok()?;
    if result.status.code() != Some(1) {
        return None;
    }
    let output = String::from_utf8(result.stdout).ok()?;
    // Use our relative headers and retain only the Git-generated hunks.
    output.find("\n@@ ").map(|start| output[start + 1..].to_owned())
}

struct TempDiff(PathBuf);

impl TempDiff {
    fn create() -> io::Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let serial = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!("ai-hub-diff-{}-{serial}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for TempDiff {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.0.join("old"));
        let _ = fs::remove_file(self.0.join("new"));
        let _ = fs::remove_dir(&self.0);
    }
}
