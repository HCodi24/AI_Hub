use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Copy the supported file auth cache only. Never copy config, plugins or sessions.
pub fn import_cli_auth(destination: &Path) -> Result<()> {
    let original = std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".codex")))
        .context("Không xác định được home Codex cá nhân; hãy đăng nhập qua web")?;
    let source = original.join("auth.json");
    if !source.is_file() {
        bail!("Không có đăng nhập CLI dạng file để dùng lại; hãy đăng nhập qua web");
    }
    let raw =
        std::fs::read(&source).context("Không đọc được đăng nhập CLI; hãy đăng nhập qua web")?;
    let value: serde_json::Value =
        serde_json::from_slice(&raw).context("Đăng nhập CLI không hợp lệ")?;
    let auth = filtered_auth(&value)?;
    crate::core::fs::write_atomic(
        &destination.join("auth.json"),
        &serde_json::to_vec(&auth)?,
        true,
    )
}

pub(super) fn filtered_auth(value: &serde_json::Value) -> Result<serde_json::Value> {
    let tokens = &value["tokens"];
    if !tokens["access_token"]
        .as_str()
        .is_some_and(|s| !s.is_empty())
        || !tokens["refresh_token"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        bail!("CLI chưa có đăng nhập ChatGPT có thể dùng lại; hãy đăng nhập qua web");
    }
    let mut auth = serde_json::Map::new();
    for name in ["auth_mode", "tokens", "last_refresh"] {
        if let Some(v) = value.get(name) {
            auth.insert(name.into(), v.clone());
        }
    }
    Ok(auth.into())
}
