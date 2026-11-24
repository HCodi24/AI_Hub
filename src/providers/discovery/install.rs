use super::{safe_cli_executable, version_dir_key};
use std::{
    env,
    path::{Path, PathBuf},
    process::Command as StdCommand,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn newest_claude_app_install() -> Option<PathBuf> {
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

pub(super) fn well_known_directories() -> Vec<PathBuf> {
    [
        env::var_os("APPDATA").map(|path| PathBuf::from(path).join("npm")),
        env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("Programs").join("nodejs")),
        env::var_os("USERPROFILE").map(|path| PathBuf::from(path).join(".local").join("bin")),
        // Antigravity CLI's own installer drops `agy.exe` here and edits PATH only for new terminals.
        env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("agy").join("bin")),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(super) fn package_npm_directories(local: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(local.join("Packages"))
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .take(200)
        .map(|package| package.path().join("LocalCache/Roaming/npm"))
}

pub(super) fn newest_codex_install() -> Option<PathBuf> {
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

pub(super) fn registered_codex_cli() -> Option<PathBuf> {
    use std::os::windows::process::CommandExt;

    let output = StdCommand::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command",
            "[Console]::OutputEncoding = [Text.Encoding]::UTF8; Get-AppxPackage -Name OpenAI.Codex | Where-Object PackageFamilyName -eq 'OpenAI.Codex_2p2nqsd0c76g0' | Sort-Object Version -Descending | Select-Object -ExpandProperty InstallLocation"])
        .creation_flags(0x08000000)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|location| !location.is_empty())
        .map(|location| Path::new(location).join("app/resources/codex.exe"))
        .find(|path| path.is_absolute() && path.is_file() && safe_cli_executable(path))
}
