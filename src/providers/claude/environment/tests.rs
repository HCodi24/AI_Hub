use super::{auth::filtered_auth, launch::enabled_plugins, *};
use crate::config::McpServerConfig;
use serde_json::{json, Value};

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
    let args: Vec<String> = launch_args(&servers, Path::new("missing-home"), Default::default())
        .iter()
        .map(|s| s.to_string_lossy().into())
        .collect();
    assert!(!args.contains(&"--disable-slash-commands".into()), "plugin skills load natively");
    assert!(args.contains(&"--strict-mcp-config".into()));
    let at = args.iter().position(|s| s == "--mcp-config").unwrap();
    let mcp: Value = serde_json::from_str(&args[at + 1]).unwrap();
    assert!(mcp["mcpServers"].get("allowed").is_some());
    assert!(mcp["mcpServers"].get("disabled").is_none());
}
#[test]
fn every_installed_plugin_is_enabled() {
    let home = std::env::temp_dir().join(format!("aihub-claude-plugins-{}", std::process::id()));
    std::fs::create_dir_all(home.join("plugins")).unwrap();
    std::fs::write(home.join("plugins/installed_plugins.json"), r#"{"plugins":{"a@m":[{}]}}"#).unwrap();
    let enabled = enabled_plugins(&home);
    std::fs::remove_dir_all(&home).ok();
    assert_eq!(enabled["a@m"], true);
}
