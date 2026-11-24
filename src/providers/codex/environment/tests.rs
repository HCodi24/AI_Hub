use super::{
    auth::filtered_auth, launch_args, mcp_servers, plugin_index, project_context, session_prefix,
    set_mcp_enabled,
};
use crate::config::{CodexConfig, ProviderConfig};
use serde_json::json;
use std::path::PathBuf;

#[test]
fn authentication_import_excludes_configuration() {
    let value = json!({"auth_mode":"chatgpt","tokens":{"access_token":"a","refresh_token":"b"},"mcp_servers":{"personal":{}},"config":"private"});
    let auth = filtered_auth(&value).unwrap();
    assert!(auth.get("mcp_servers").is_none());
    assert!(auth.get("config").is_none());
    assert!(filtered_auth(&json!({"OPENAI_API_KEY":"secret"})).is_err());
}
#[test]
fn mcp_servers_are_listed_and_toggled_in_config_toml() {
    let home = std::env::temp_dir().join(format!("aihub-codex-mcp-{}", std::process::id()));
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("config.toml"),
        concat!(
            "[mcp_servers.a]\n",
            "command = \"x\"\n",
            "[mcp_servers.a.env]\n",
            "K = \"v\"\n",
            "[mcp_servers.b]\n",
            "command = \"y\"\n",
            "enabled = false\n",
        ),
    )
    .unwrap();
    assert_eq!(mcp_servers(&home).iter().map(|s| s["enabled"] == true).collect::<Vec<_>>(), [true, false]);
    set_mcp_enabled(&home, "b", true).unwrap();
    set_mcp_enabled(&home, "a", false).unwrap();
    let after = mcp_servers(&home);
    std::fs::remove_dir_all(&home).ok();
    assert_eq!(after.iter().map(|s| s["enabled"] == true).collect::<Vec<_>>(), [false, true]);
    assert_eq!(after[0]["summary"], "x");
}
#[test]
fn plugin_index_lists_enabled_plugin_skills_only() {
    let home = std::env::temp_dir().join(format!("aihub-plugin-index-{}", std::process::id()));
    for (plugin, skill) in [("on", "alpha"), ("off", "beta")] {
        let dir = home.join(format!("plugins/cache/m/{plugin}/1.0/skills/{skill}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: {skill}\ndescription: \"does {skill}\"\n---\n"),
        )
        .unwrap();
    }
    std::fs::write(
        home.join("config.toml"),
        "[plugins.\"on@m\"]\nenabled = true\n[plugins.\"off@m\"]\nenabled = false\n",
    )
    .unwrap();
    let index = plugin_index(&home);
    std::fs::remove_dir_all(&home).ok();
    assert!(index.contains("on:alpha") && index.contains("does alpha") && !index.contains("beta"));
}
#[test]
fn permission_changes_invalidate_sessions() {
    let provider = ProviderConfig {
        id: "codex".into(),
        ..Default::default()
    };
    let mut settings = CodexConfig::default();
    let first = session_prefix(&settings, &provider, "C:/repo");
    settings.project_skills = true;
    assert_ne!(first, session_prefix(&settings, &provider, "C:/repo"));
    assert_ne!(
        first,
        session_prefix(&CodexConfig::default(), &provider, "C:/other")
    );
}

#[test]
fn project_sources_are_separate_and_disabled_by_default() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target/codex-sources-test")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::create_dir_all(root.join(".agents/skills/test")).unwrap();
    std::fs::write(root.join("AGENTS.md"), "INSTRUCTION_MARKER").unwrap();
    std::fs::write(root.join(".agents/skills/test/SKILL.md"), "SKILL_MARKER").unwrap();
    let mut settings = CodexConfig::default();
    assert_eq!(project_context(&root, &settings).unwrap(), "");
    settings.project_skills = true;
    let skills = project_context(&root, &settings).unwrap();
    assert!(skills.contains("SKILL_MARKER"));
    assert!(!skills.contains("INSTRUCTION_MARKER"));
    settings.project_skills = false;
    settings.project_instructions = true;
    let instructions = project_context(&root, &settings).unwrap();
    assert!(!instructions.contains("SKILL_MARKER"));
    assert!(instructions.contains("INSTRUCTION_MARKER"));
    let args = launch_args(&root, false)
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert!(args.contains(&"features.plugins=false".to_owned()));
    assert!(launch_args(&root, true)
        .iter()
        .any(|arg| arg == "features.plugins=true"));
    assert!(launch_args(&root, true)
        .iter()
        .any(|arg| arg == "skills.include_instructions=true"));
    assert!(args.contains(&"project_doc_max_bytes=0".to_owned()));
    assert!(args.contains(&"skills.include_instructions=false".to_owned()));
}
