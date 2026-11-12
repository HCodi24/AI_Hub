use crate::config::{ClaudeConfig, McpServerConfig, ProviderConfig};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub fn home(config: &Path, provider: &ProviderConfig) -> Result<PathBuf> {
    if provider.id.is_empty()
        || provider.id.len() > 80
        || !provider
            .id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        bail!("Mã nguồn Claude không hợp lệ");
    }
    let data = crate::core::paths::data_dir(config)?;
    let dir = data.join("claude").join(&provider.id);
    std::fs::create_dir_all(&dir)?;
    // Migrate only the original default source, once. Never restore credentials after logout.
    let marker = dir.join(".mutis-migrated");
    if !marker.exists() {
        if provider.id == "claude-cli" && !dir.join(".credentials.json").exists() {
            let old = data.join("claude-config/.credentials.json");
            if old.is_file() {
                import_auth_file(&old, &dir)?;
            }
        }
        std::fs::write(marker, b"1")?;
    }
    Ok(dir)
}

pub fn settings() -> Result<ClaudeConfig> {
    let config: crate::config::Config =
        serde_json::from_slice(&std::fs::read(crate::config::config_file_path()?)?)?;
    Ok(config.claude)
}

pub fn session_prefix(settings: &ClaudeConfig, provider: &ProviderConfig, folder: &str) -> String {
    let text = format!(
        "{}:{}:{}:{}:{}:{}",
        settings.revision,
        settings.project_skills,
        settings.project_instructions,
        provider.id,
        provider.executable,
        folder
    );
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    let hash: String = digest.as_ref()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("mutis-claude-{hash}:")
}

pub fn launch_args(mcp: &[McpServerConfig]) -> Vec<OsString> {
    let servers = mcp
        .iter()
        .filter(|s| s.enabled)
        .map(|s| {
            (
                s.name.clone(),
                json!({"command":s.command,"args":s.args,"env":s.env}),
            )
        })
        .collect::<serde_json::Map<String, Value>>();
    vec!["--setting-sources".into(), "user".into(), "--disable-slash-commands".into(),
        "--strict-mcp-config".into(), "--mcp-config".into(), json!({"mcpServers":servers}).to_string().into(),
        "--settings".into(), json!({"disableAllHooks":true,"autoMemoryEnabled":false,"enabledPlugins":{},"claudeMdExcludes":["**"],"disableClaudeAiConnectors":true}).to_string().into()]
}

pub fn apply(command: &mut tokio::process::Command, home: &Path) {
    crate::providers::cli_support::clean_claude_environment(command);
    command.env("CLAUDE_CONFIG_DIR", home);
}

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
    write_private(
        &destination.join(".credentials.json"),
        &serde_json::to_vec(&filtered)?,
    )
}

fn write_private(path: &Path, data: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let result = (|| -> Result<()> {
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

fn filtered_auth(value: &Value) -> Result<Value> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auth_import_only_preserves_supported_oauth_fields() {
        let input = json!({"claudeAiOauth":{"accessToken":"a","refreshToken":"b","hooks":{}},"mcpServers":{},"apiKey":"secret"});
        let result = filtered_auth(&input).unwrap();
        assert_eq!(
            result,
            json!({"claudeAiOauth":{"accessToken":"a","refreshToken":"b"}})
        );
        assert!(filtered_auth(&json!({"apiKey":"a"})).is_err());
    }
    #[test]
    fn source_changes_invalidate_sessions() {
        let p = ProviderConfig {
            id: "claude-cli".into(),
            ..Default::default()
        };
        let mut s = ClaudeConfig::default();
        let first = session_prefix(&s, &p, "repo");
        s.project_skills = true;
        assert_ne!(first, session_prefix(&s, &p, "repo"));
        s.project_skills = false;
        s.revision += 1;
        assert_ne!(first, session_prefix(&s, &p, "repo"));
    }

    #[test]
    fn legacy_credentials_migrate_once_without_copying_settings() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/claude-migration-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(root.join("claude-config")).unwrap();
        std::fs::write(
            root.join("claude-config/.credentials.json"),
            json!({"claudeAiOauth":{"accessToken":"a","refreshToken":"b"}}).to_string(),
        )
        .unwrap();
        std::fs::write(root.join("claude-config/settings.json"), "{\"hooks\":{}}").unwrap();
        let provider = ProviderConfig {
            id: "claude-cli".into(),
            ..Default::default()
        };
        let dir = home(&root.join("config.json"), &provider).unwrap();
        assert!(dir.join(".credentials.json").is_file());
        assert!(!dir.join("settings.json").exists());
        std::fs::remove_file(dir.join(".credentials.json")).unwrap();
        home(&root.join("config.json"), &provider).unwrap();
        assert!(!dir.join(".credentials.json").exists());
        let other = ProviderConfig {
            id: "another".into(),
            ..Default::default()
        };
        assert!(!home(&root.join("config.json"), &other)
            .unwrap()
            .join(".credentials.json")
            .exists());
        let invalid = ProviderConfig {
            id: "../escape".into(),
            ..Default::default()
        };
        assert!(home(&root.join("config.json"), &invalid).is_err());
    }

    #[test]
    fn launch_only_allows_app_managed_mcp_and_disables_implicit_sources() {
        let servers = vec![
            McpServerConfig {
                name: "allowed".into(),
                command: "server".into(),
                args: vec![],
                env: Default::default(),
                enabled: true,
            },
            McpServerConfig {
                name: "disabled".into(),
                command: "other".into(),
                args: vec![],
                env: Default::default(),
                enabled: false,
            },
        ];
        let args: Vec<String> = launch_args(&servers)
            .iter()
            .map(|s| s.to_string_lossy().into())
            .collect();
        assert!(args.contains(&"--disable-slash-commands".into()));
        assert!(args.contains(&"--strict-mcp-config".into()));
        let at = args.iter().position(|s| s == "--mcp-config").unwrap();
        let mcp: Value = serde_json::from_str(&args[at + 1]).unwrap();
        assert!(mcp["mcpServers"].get("allowed").is_some());
        assert!(mcp["mcpServers"].get("disabled").is_none());
    }
}
