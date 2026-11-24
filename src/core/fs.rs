use std::path::Path;

#[cfg(test)]
mod tests;

/// Writes through a temporary file next to the destination, then renames it into place.
/// On Unix, private files are created with owner-only permissions.
pub fn write_atomic(path: &Path, data: &[u8], private: bool) -> anyhow::Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    #[cfg(not(unix))]
    let _ = private;
    let result = (|| -> anyhow::Result<()> {
        let mut file = options.open(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(tmp);
    }
    result
}
