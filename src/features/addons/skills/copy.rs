use super::super::paths::{checked_metadata, safe_component};
use anyhow::{bail, Context, Result};
use std::{ffi::OsStr, fs, path::Path};

pub(in crate::features::addons) fn copy_checkout_dir(from: &Path, to: &Path) -> Result<()> {
    let mut directories = vec![(from.to_path_buf(), to.to_path_buf())];
    while let Some((from, to)) = directories.pop() {
        fs::create_dir(&to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            let name = entry.file_name();
            let source = entry.path();
            if !safe_component(&name) {
                bail!("Tên tệp hoặc thư mục không an toàn «{}»", source.display());
            }
            let metadata = checked_metadata(&source)?.context("Tệp trong skill đã biến mất")?;
            if name.eq_ignore_ascii_case(OsStr::new(".git")) {
                continue;
            }
            let destination = to.join(name);
            if metadata.is_dir() {
                directories.push((source, destination));
            } else if metadata.is_file() {
                if checked_metadata(&destination)?.is_some() {
                    bail!(
                        "Hai tệp trong skill có cùng đường dẫn đích «{}»",
                        destination.display()
                    );
                }
                fs::copy(&source, &destination)?;
            } else {
                bail!(
                    "Skill chứa loại tệp không được hỗ trợ «{}»",
                    source.display()
                );
            }
        }
    }
    Ok(())
}
