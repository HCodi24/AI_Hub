use super::project_root;
use serde_json::json;
use std::{ffi::OsString, path::Path};

/// Codex plugins are off unless the user installed some into AI Hub's own Codex home (Settings › Add-ons).
pub fn has_plugins(home: &Path) -> bool {
    std::fs::read_dir(home.join("plugins/cache")).is_ok_and(|mut entries| entries.next().is_some())
}

pub fn launch_args(folder: &Path, plugins: bool) -> Vec<OsString> {
    let root = project_root(folder);
    let configs = [
        "cli_auth_credentials_store=\"file\"".to_owned(),
        // Without a Windows sandbox mode Codex falls back to read-only no matter what permission is requested.
        "windows.sandbox=\"elevated\"".into(),
        "project_doc_max_bytes=0".into(),
        // With plugins installed, let Codex list their skills itself (as Orca does); without any it stays off so repo skills are never auto-loaded.
        format!("skills.include_instructions={plugins}"),
        "skills.bundled.enabled=false".into(),
        "features.apps=false".into(),
        format!("features.plugins={plugins}"),
        "memories.use_memories=false".into(),
        format!(
            "projects.{}.trust_level=\"untrusted\"",
            json!(root.to_string_lossy())
        ),
    ];
    let mut args = vec!["app-server".into(), "--listen".into(), "stdio://".into()];
    for config in configs {
        args.extend([OsString::from("-c"), config.into()]);
    }
    args
}
