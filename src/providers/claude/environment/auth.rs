use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub fn import_cli_auth(destination: &Path) -> Result<()> {
    let original = std::env::var_os("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".claude")))
        .context("Không tìm thấy cấu hình Claude CLI")?;
    import_auth_file(&original.join(".credentials.json"), destination)
}

pub fn import_auth_file(source: &Path, destination: &Path) -> Result<()> {
    let bytes = std::fs::read(source)
        .context("Không có đăng nhập Claude dạng file tương thích; hãy đăng nhập qua web")?;
    let filtered = filtered_auth(
        &serde_json::from_slice::<Value>(&bytes).context("Đăng nhập Claude không hợp lệ")?,
    )?;
    crate::core::fs::write_atomic(
        &destination.join(".credentials.json"),
        &serde_json::to_vec(&filtered)?,
        true,
    )
}

pub(super) fn filtered_auth(value: &Value) -> Result<Value> {
    let oauth = &value["claudeAiOauth"];
    if !oauth["accessToken"].as_str().is_some_and(|s| !s.is_empty())
        || !oauth["refreshToken"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
    {
        bail!("Claude CLI chưa có đăng nhập có thể dùng lại; hãy đăng nhập qua web");
    }
    let mut fields = serde_json::Map::new();
    for key in [
        "accessToken",
        "refreshToken",
        "expiresAt",
        "scopes",
        "subscriptionType",
        "rateLimitTier",
    ] {
        if let Some(value) = oauth.get(key) {
            fields.insert(key.into(), value.clone());
        }
    }
    Ok(json!({"claudeAiOauth":fields}))
}
