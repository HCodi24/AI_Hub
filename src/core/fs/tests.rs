use super::write_atomic;
use std::{fs, path::PathBuf};

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("ai-hub-atomic-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn write_atomic_creates_and_replaces_without_leaving_a_temp_file() {
    let dir = TempDir::new();
    let path = dir.0.join("data.json");

    write_atomic(&path, b"first version", false).unwrap();
    assert_eq!(fs::read(&path).unwrap().as_slice(), b"first version");
    write_atomic(&path, b"new", false).unwrap();
    assert_eq!(fs::read(&path).unwrap().as_slice(), b"new");
    assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn private_atomic_write_sets_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = TempDir::new();
    let path = dir.0.join("auth.json");
    write_atomic(&path, b"credentials", true).unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}
