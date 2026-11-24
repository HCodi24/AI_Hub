use anyhow::{bail, Context, Result};
use std::path::{Component, Path, PathBuf};

pub(in crate::providers::agent) fn safe_path(root: &Path, raw: &str) -> Result<PathBuf> {
    if raw.starts_with('/') || raw.starts_with('\\') || raw.as_bytes().get(1) == Some(&b':') {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let normalized = raw.replace('\\', "/");
    if normalized.split('/').any(|part| part == "..") {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    let rel = Path::new(&normalized);
    if rel.as_os_str().is_empty()
        || rel.is_absolute()
        || rel
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!("Đường dẫn phải tương đối và không được chứa ..");
    }
    if rel.components().any(|component| {
        [".git", "target", "node_modules", ".codegraph"]
            .iter()
            .any(|blocked| {
                component
                    .as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case(blocked)
            })
    }) {
        bail!("Đường dẫn thuộc vùng bị cấm");
    }
    let candidate = root.join(rel);
    let mut ancestor = candidate.as_path();
    let mut suffix = Vec::new();
    while !ancestor.exists() {
        suffix.push(
            ancestor
                .file_name()
                .context("Đường dẫn không hợp lệ")?
                .to_os_string(),
        );
        ancestor = ancestor.parent().context("Đường dẫn không hợp lệ")?;
    }
    let mut check = ancestor.canonicalize()?;
    for part in suffix.iter().rev() {
        check.push(part);
    }
    if check != root && !check.starts_with(root) {
        bail!("Đường dẫn vượt khỏi thư mục chat");
    }
    Ok(candidate)
}
