use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Command as StdCommand, Stdio},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tokio::time::timeout;

use super::cli_support;

pub fn resolve_executable(name: &str, explicit_path: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = explicit_path.map(str::trim).filter(|path| !path.is_empty()) {
        let path = PathBuf::from(path);
        if path.is_file() && safe_cli_executable(&path) {
            return Some(path);
        }
    }

    let names = [
        format!("{name}.exe"),
        format!("{name}.cmd"),
        name.to_owned(),
    ];
    let process_path = env::var_os("PATH").into_iter().flat_map(|value| {
        env::split_paths(&value)
            .filter(|path| valid_path_entry(path))
            .collect::<Vec<_>>()
    });
    if let Some(path) = find_in_directories(&names, process_path) {
        return Some(path);
    }

    #[cfg(windows)]
    {
        if let Some(path) = find_in_directories(&names, registry_path_directories()) {
            return Some(path);
        }

        let directories = well_known_directories();
        if let Some(path) = find_in_directories(&names, directories) {
            return Some(path);
        }

        if name.eq_ignore_ascii_case("codex") {
            if let Some(path) = newest_codex_install() {
                return Some(path);
            }
        }
        if name.eq_ignore_ascii_case("claude") {
            if let Some(path) = newest_claude_app_install() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(windows)]
fn newest_claude_app_install() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("LOCALAPPDATA")?).join("Claude-Data");
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|install| {
            if !install.file_type().ok()?.is_dir() {
                return None;
            }
            let versions = install.path().join("claude-code");
            Some(
                std::fs::read_dir(versions)
                    .ok()?
                    .filter_map(Result::ok)
                    .filter_map(|version| {
                        if !version.file_type().ok()?.is_dir() {
                            return None;
                        }
                        let version_name = version.file_name().to_string_lossy().into_owned();
                        let parsed = version_dir_key(&version_name)?;
                        let hashes = std::fs::read_dir(version.path()).ok()?;
                        hashes.filter_map(Result::ok).find_map(|hash| {
                            let executable = hash.path().join("claude.exe");
                            (hash.file_type().ok()?.is_dir()
                                && executable.is_file()
                                && safe_cli_executable(&executable))
                            .then_some((parsed.clone(), executable))
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .flatten()
        .collect::<Vec<_>>();
    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    candidates.into_iter().next().map(|(_, path)| path)
}

pub fn version_dir_key(version: &str) -> Option<(Vec<u64>, bool, String)> {
    let (release, prerelease) = version.split_once('-').unwrap_or((version, ""));
    let mut numbers = Vec::new();
    for part in release.split('.') {
        numbers.push(part.parse::<u64>().ok()?);
    }
    if numbers.len() != 3 || version.contains('+') || prerelease.contains('+') {
        return None;
    }
    Some((numbers, prerelease.is_empty(), prerelease.to_owned()))
}

fn valid_path_entry(path: &Path) -> bool {
    let path = path.to_string_lossy();
    !path.trim().is_empty() && !path.contains('$') && !path.contains('%')
}

fn find_in_directories(
    names: &[String],
    directories: impl IntoIterator<Item = PathBuf>,
) -> Option<PathBuf> {
    let directories = directories
        .into_iter()
        .filter(|directory| valid_path_entry(directory))
        .collect::<Vec<_>>();
    for name in names {
        for directory in &directories {
            let candidate = directory.join(name);
            if candidate.is_file() && safe_cli_executable(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn safe_cli_executable(path: &Path) -> bool {
    let lower = path.to_string_lossy().to_lowercase();
    if lower.contains("\\program files\\windowsapps\\") {
        return false;
    }
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("Claude.exe"))
    {
        return !path
            .parent()
            .is_some_and(|parent| parent.join("resources").join("app.asar").exists());
    }
    true
}

#[cfg(windows)]
fn registry_path_directories() -> Vec<PathBuf> {
    [
        r"HKCU\Environment",
        r"HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment",
    ]
    .into_iter()
    .filter_map(read_registry_path)
    .flat_map(|path| env::split_paths(std::ffi::OsStr::new(&path)).collect::<Vec<_>>())
    .filter(|path| valid_path_entry(path))
    .collect()
}

#[cfg(windows)]
fn read_registry_path(key: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let output = StdCommand::new("reg")
        .args(["query", key, "/v", "Path"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let output = String::from_utf8_lossy(&output.stdout);
    output.lines().find_map(|line| {
        let mut fields = line.split_whitespace();
        let name = fields.next()?;
        let value_type = fields.next()?;
        if !name.eq_ignore_ascii_case("Path") || !matches!(value_type, "REG_SZ" | "REG_EXPAND_SZ") {
            return None;
        }
        let value_start = line.find(value_type)? + value_type.len();
        let value = line[value_start..].trim();
        Some(expand_windows_variables(value))
    })
}

#[cfg(windows)]
fn expand_windows_variables(value: &str) -> String {
    let mut expanded = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('%') {
        expanded.push_str(&rest[..start]);
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('%') else {
            expanded.push_str(&rest[start..]);
            return expanded;
        };
        let variable = &after_start[..end];
        if let Some(value) = env::var_os(variable) {
            expanded.push_str(&value.to_string_lossy());
        } else {
            expanded.push('%');
            expanded.push_str(variable);
            expanded.push('%');
        }
        rest = &after_start[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

#[cfg(windows)]
fn well_known_directories() -> Vec<PathBuf> {
    [
        env::var_os("APPDATA").map(|path| PathBuf::from(path).join("npm")),
        env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Programs").join("nodejs")),
        env::var_os("USERPROFILE").map(|path| PathBuf::from(path).join(".local").join("bin")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

#[cfg(windows)]
fn newest_codex_install() -> Option<PathBuf> {
    let root = PathBuf::from(env::var_os("LOCALAPPDATA")?)
        .join("OpenAI")
        .join("Codex")
        .join("bin");
    let mut candidates = std::fs::read_dir(root)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().ok()?.is_dir() {
                return None;
            }
            let executable = entry.path().join("codex.exe");
            if !executable.is_file() || !safe_cli_executable(&executable) {
                return None;
            }
            let modified = executable
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            Some((modified, executable))
        })
        .collect::<Vec<(SystemTime, PathBuf)>>();
    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    candidates.into_iter().next().map(|(_, path)| path)
}

fn unsafe_cli_path(path: &std::path::Path) -> bool {
    let text = path.to_string_lossy().to_lowercase();
    if text.contains("\\program files\\windowsapps\\") {
        return true;
    }
    if path
        .file_name()
        .is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case("Claude.exe"))
    {
        return path
            .parent()
            .is_some_and(|p| p.join("resources").join("app.asar").exists());
    }
    false
}
pub(crate) async fn run_cli_version(path: &std::path::Path, limit: Duration) -> Option<String> {
    let mut command = cli_support::command_for(path, &[OsString::from("--version")]);
    if path
        .file_name()
        .is_some_and(|name| name.to_string_lossy().eq_ignore_ascii_case("claude.exe"))
        || path.to_string_lossy().to_lowercase().contains("claude")
    {
        cli_support::clean_claude_environment(&mut command);
    }
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = timeout(limit, command.output()).await.ok()?.ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let value = if stdout.trim().is_empty() {
        stderr.trim()
    } else {
        stdout.trim()
    };
    (!value.is_empty()).then(|| value.chars().take(240).collect())
}

pub(crate) async fn probe_cli_info(
    name: &str,
    explicit_path: &str,
) -> (bool, Option<String>, Option<String>) {
    let Some(path) = resolve_executable(name, Some(explicit_path.trim())) else {
        return (false, None, None);
    };
    let path_text = path.to_string_lossy().into_owned();
    let version = run_cli_version(&path, Duration::from_secs(5)).await;
    (true, version, Some(path_text))
}

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
        if unsafe_cli_path(&path) {
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
fn drop_bundled_cli_duplicates(candidates: Vec<CliCandidate>) -> Vec<CliCandidate> {
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
            let root = l.join("Packages");
            if let Ok(packages) = std::fs::read_dir(root) {
                for package in packages.flatten().take(200) {
                    add(
                        package
                            .path()
                            .join(format!("LocalCache/Roaming/npm/{name}.cmd")),
                        "package npm",
                    );
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod cli_discovery_tests {
    use super::{drop_bundled_cli_duplicates, CliCandidate};

    #[test]
    fn drops_same_version_binary_inside_cmd_node_modules() {
        let candidates = vec![
            CliCandidate {
                path: r"C:\Users\test\npm\codex.cmd".into(),
                version: "codex-cli 0.167.0".into(),
                source: "PATH".into(),
            },
            CliCandidate {
                path: r"C:\Users\test\npm\node_modules\@openai\codex\node_modules\@openai\codex-win32-x64\vendor\x86_64-pc-windows-msvc\bin\codex.exe".into(),
                version: "codex-cli 0.167.0".into(),
                source: "npm bundled".into(),
            },
            CliCandidate {
                path: r"C:\Other\codex.exe".into(),
                version: "codex-cli 0.167.0".into(),
                source: "other".into(),
            },
            CliCandidate {
                path: r"C:\Users\test\npm\node_modules\codex-next\bin\codex.exe".into(),
                version: "codex-cli 0.168.0".into(),
                source: "npm bundled".into(),
            },
        ];

        let retained = drop_bundled_cli_duplicates(candidates);
        assert_eq!(retained.len(), 3);
        assert!(retained
            .iter()
            .all(|candidate| !candidate.path.contains("codex-win32-x64")));
    }
}
