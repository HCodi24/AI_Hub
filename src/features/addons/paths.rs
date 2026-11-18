use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    ffi::OsStr,
    fs::{self, Metadata},
    io::ErrorKind,
    path::Path,
};

/// Plugin ids look like `name@marketplace`; skill and MCP names are plain. Anything else is refused (they reach a
/// command line and a file path).
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with(['-', '.'])
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '@'))
}

pub(super) fn valid_skill_name(name: &str) -> bool {
    valid_name(name) && !name.eq_ignore_ascii_case("synced") && safe_component(OsStr::new(name))
}

pub(crate) fn safe_component(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return false;
    };
    if name.is_empty()
        || name.ends_with([' ', '.'])
        || name.chars().any(|c| {
            c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
    {
        return false;
    }
    let stem = name.split('.').next().unwrap_or_default().trim_end();
    let stem = stem.to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|suffix| {
            (suffix.len() == 1 && suffix.as_bytes()[0].is_ascii_digit())
                || matches!(suffix, "¹" | "²" | "³")
        })
}

pub(super) fn checked_metadata(path: &Path) -> Result<Option<Metadata>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Không đọc được «{}»", path.display()))
        }
    };
    #[cfg(windows)]
    let reparse_point = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    #[cfg(not(windows))]
    let reparse_point = false;
    if metadata.is_symlink() || reparse_point {
        bail!(
            "Đường dẫn «{}» là liên kết hoặc điểm phân tích lại",
            path.display()
        );
    }
    Ok(Some(metadata))
}

pub(super) fn check_directories(path: &Path) -> Result<()> {
    let ancestors: Vec<_> = path
        .ancestors()
        .filter(|path| !path.as_os_str().is_empty())
        .collect();
    for path in ancestors.into_iter().rev() {
        if checked_metadata(path)?.is_some_and(|metadata| !metadata.is_dir()) {
            bail!("«{}» phải là thư mục", path.display());
        }
    }
    Ok(())
}

pub(super) fn read_json(path: &Path) -> Value {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or(Value::Null)
}
