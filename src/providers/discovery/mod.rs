use std::{
    env,
    path::{Path, PathBuf},
};

mod candidates;
#[cfg(windows)]
mod install;
mod probe;
#[cfg(windows)]
mod windows;

pub(crate) use candidates::{discover_candidates, CliCandidate};
pub(crate) use probe::{probe_cli_info, run_cli_version};

#[cfg(windows)]
use install::{
    newest_claude_app_install, newest_codex_install, package_npm_directories,
    registered_codex_cli, well_known_directories,
};
#[cfg(windows)]
use windows::registry_path_directories;

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
        if let Some(local) = env::var_os("LOCALAPPDATA") {
            let cmd = format!("{name}.cmd");
            if let Some(path) = find_in_directories(&[cmd], package_npm_directories(Path::new(&local))) {
                return Some(path);
            }
        }
        if name.eq_ignore_ascii_case("codex") {
            return registered_codex_cli();
        }
    }

    None
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

pub(super) fn valid_path_entry(path: &Path) -> bool {
    let path = path.to_string_lossy();
    !path.trim().is_empty() && !path.contains('$') && !path.contains('%')
}

pub(super) fn find_in_directories(
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

pub(super) fn safe_cli_executable(path: &Path) -> bool {
    let lower = path.to_string_lossy().replace('/', "\\").to_lowercase();
    if let Some((_, relative)) = lower.split_once("\\program files\\windowsapps\\") {
        let Some((package, executable)) = relative.split_once('\\') else {
            return false;
        };
        return package.starts_with("openai.codex_")
            && package.ends_with("__2p2nqsd0c76g0")
            && executable == r"app\resources\codex.exe";
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

#[cfg(test)]
mod tests;
