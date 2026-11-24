use std::path::Path;

/// Key of a project folder in `mutis.verify_commands`: forward slashes, no trailing slash, lower-case on Windows.
pub fn folder_key(folder: &Path) -> String {
    let key = folder.to_string_lossy().replace('\\', "/");
    let key = key.trim_end_matches('/');
    if cfg!(windows) { key.to_lowercase() } else { key.to_owned() }
}

/// The command this folder is verified with: its own saved one, else the global one ("" = auto-detect).
pub fn command_for(mutis: &crate::config::MutisConfig, folder: &Path) -> String {
    mutis.verify_commands.get(&folder_key(folder)).filter(|c| !c.trim().is_empty()).cloned().unwrap_or_else(|| mutis.verify_command.clone())
}

/// What auto-detection would run in this folder, as (name, command).
pub fn detected_commands(folder: &Path) -> Vec<(String, String)> {
    detect(folder).into_iter().map(|(name, command)| (name.to_owned(), command)).collect()
}

pub(super) fn detect(folder: &Path) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let powershell_script = folder.join("scripts/verify.ps1");
    let script = folder.join("scripts/verify/verify.sh");
    if powershell_script.is_file() && command_available("powershell") {
        out.push((
            "verify_script",
            format!(
                "powershell -NoProfile -ExecutionPolicy Bypass -File \"{}\"",
                powershell_script.display()
            ),
        ));
    } else if script.is_file() && command_available("bash") {
        out.push(("verify_script", format!("bash \"{}\"", script.display())));
    } else if folder.join("Cargo.toml").is_file() {
        out.push(("cargo build", "cargo build --offline".into()));
        out.push(("cargo test", "cargo test --offline".into()));
        if command_available("cargo-clippy") {
            out.push((
                "cargo clippy",
                "cargo clippy --offline --all-targets -- -D warnings".into(),
            ));
        }
    } else if folder.join("package.json").is_file() {
        if let Ok(text) = std::fs::read_to_string(folder.join("package.json")) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                for key in ["typecheck", "lint", "build", "test"] {
                    if value["scripts"][key].is_string() {
                        out.push(("npm script", format!("npm run -s {key}")));
                    }
                }
            }
        }
    }
    out
}
pub(super) fn command_available(name: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths).any(|p| {
            p.join(if cfg!(windows) {
                format!("{name}.exe")
            } else {
                name.into()
            })
            .exists()
        })
    })
}
