#[cfg(windows)]
use super::{install::package_npm_directories, version_dir_key};
use super::{resolve_executable, run_cli_version, safe_cli_executable};
use serde::Serialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::time::timeout;

#[derive(Serialize)]
pub(crate) struct CliCandidate {
    pub(crate) path: String,
    pub(crate) version: String,
    pub(crate) source: String,
}
pub(crate) async fn discover_candidates(name: &str) -> Vec<CliCandidate> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let search_name = name.to_owned();
    let found = timeout(
        deadline.saturating_duration_since(Instant::now()),
        tokio::task::spawn_blocking(move || discovery_paths(&search_name)),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .unwrap_or_default();
    let mut results = Vec::new();
    for (path, source) in found.into_iter().take(200) {
        if Instant::now() >= deadline {
            break;
        }
        if !safe_cli_executable(&path) {
            continue;
        }
        if let Some(version) = run_cli_version(
            &path,
            (deadline - Instant::now()).min(Duration::from_secs(10)),
        )
        .await
        {
            let matches = if name == "codex" {
                version.to_lowercase().contains("codex")
            } else {
                version.contains("Claude Code")
            };
            if matches
                && !results.iter().any(|c: &CliCandidate| {
                    c.version == version
                        && canonical_install(PathBuf::from(&c.path))
                            == canonical_install(path.clone())
                })
            {
                let path_text = path.to_string_lossy().replace('/', "\\");
                results.push(CliCandidate {
                    path: path_text,
                    version,
                    source,
                });
            }
        }
    }
    drop_bundled_cli_duplicates(results)
}
pub(super) fn drop_bundled_cli_duplicates(candidates: Vec<CliCandidate>) -> Vec<CliCandidate> {
    let command_candidates = candidates
        .iter()
        .filter(|candidate| {
            PathBuf::from(&candidate.path)
                .extension()
                .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("cmd"))
        })
        .map(|candidate| {
            (
                candidate.version.clone(),
                PathBuf::from(&candidate.path)
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new(""))
                    .join("node_modules"),
            )
        })
        .collect::<Vec<_>>();

    candidates
        .into_iter()
        .filter(|candidate| {
            let path = PathBuf::from(&candidate.path);
            !command_candidates.iter().any(|(version, node_modules)| {
                if candidate.version != *version {
                    return false;
                }
                let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
                let node_modules = node_modules
                    .to_string_lossy()
                    .replace('/', "\\")
                    .trim_end_matches('\\')
                    .to_lowercase();
                path.starts_with(&format!("{node_modules}\\"))
            })
        })
        .collect()
}
fn canonical_install(path: PathBuf) -> PathBuf {
    path.canonicalize()
        .map(crate::core::text::strip_verbatim)
        .unwrap_or(path)
}
fn discovery_paths(name: &str) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let mut add = |p: PathBuf, s: &str| {
        if p.is_file() && !out.iter().any(|(x, _)| x == &p) {
            out.push((p, s.into()));
        }
    };
    for dir in std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()) {
        for n in [format!("{name}.cmd"), format!("{name}.exe"), name.into()] {
            add(dir.join(n), "PATH");
        }
    }
    if let Some(path) = resolve_executable(name, None) {
        add(path, "resolved executable");
    }
    #[cfg(windows)]
    {
        let app = std::env::var_os("APPDATA").map(PathBuf::from);
        let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let user = std::env::var_os("USERPROFILE").map(PathBuf::from);
        if let Some(a) = app.as_ref() {
            add(a.join("npm").join(format!("{name}.cmd")), "npm");
            if name == "codex" {
                let vendor = a
                    .join("npm")
                    .join("node_modules")
                    .join("@openai")
                    .join("codex")
                    .join("node_modules")
                    .join("@openai")
                    .join("codex-win32-x64")
                    .join("vendor");
                if let Ok(entries) = std::fs::read_dir(vendor) {
                    for e in entries.flatten().take(200) {
                        add(e.path().join("bin/codex.exe"), "npm bundled");
                    }
                }
            }
        }
        if let Some(u) = user {
            if name == "claude" {
                add(
                    u.join(".local").join("bin").join("claude.exe"),
                    "user local bin",
                );
                add(
                    u.join(".claude").join("local").join("claude"),
                    "Claude Code",
                );
                add(
                    u.join(".claude").join("local").join("claude.exe"),
                    "Claude Code",
                );
                add(
                    u.join(".claude").join("local").join("claude.cmd"),
                    "Claude Code",
                );
            }
        }
        if let Some(l) = local.as_ref() {
            if name == "claude" {
                let root = l.join("Claude-Data");
                let mut app_candidates = Vec::new();
                if let Ok(installs) = std::fs::read_dir(root) {
                    for install in installs.flatten().take(200) {
                        let versions = install.path().join("claude-code");
                        if let Ok(entries) = std::fs::read_dir(versions) {
                            for version in entries.flatten().take(200) {
                                let Some(parsed) =
                                    version_dir_key(&version.file_name().to_string_lossy())
                                else {
                                    continue;
                                };
                                if let Ok(hashes) = std::fs::read_dir(version.path()) {
                                    for hash in hashes.flatten().take(200) {
                                        let executable = hash.path().join("claude.exe");
                                        if executable.is_file() {
                                            app_candidates.push((parsed.clone(), executable));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                app_candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
                for (_, path) in app_candidates {
                    add(path, "Claude app");
                }
            }
            if name == "codex" {
                let root = l.join("OpenAI/Codex/bin");
                if let Ok(entries) = std::fs::read_dir(root) {
                    for e in entries.flatten().take(200) {
                        add(e.path().join("codex.exe"), "Codex app");
                    }
                }
            }
        }
        if let Some(l) = local {
            for directory in package_npm_directories(&l) {
                add(directory.join(format!("{name}.cmd")), "package npm");
            }
        }
    }
    out
}
