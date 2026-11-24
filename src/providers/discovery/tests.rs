use super::candidates::{drop_bundled_cli_duplicates, CliCandidate};

#[test]
fn allows_only_the_codex_cli_in_windowsapps() {
    use super::safe_cli_executable;
    use std::path::Path;

    assert!(safe_cli_executable(Path::new(
        r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1117.4957.0_x64__2p2nqsd0c76g0\app\resources\codex.exe",
    )));
    assert!(!safe_cli_executable(Path::new(
        r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1117.4957.0_x64__2p2nqsd0c76g0\app\Codex.exe",
    )));
    assert!(!safe_cli_executable(Path::new(
        r"C:\Program Files\WindowsApps\OpenAI.Codex_26.1117.4957.0_x64__other\app\resources\codex.exe",
    )));
}

#[cfg(windows)]
#[test]
fn finds_npm_shim_in_windows_package_cache() {
    use super::{find_in_directories, install::package_npm_directories};
    use std::{env, fs};

    let local = env::temp_dir().join(format!("mutis-cli-test-{}", uuid::Uuid::new_v4()));
    let npm = local.join("Packages/Claude_test/LocalCache/Roaming/npm");
    fs::create_dir_all(&npm).unwrap();
    let shim = npm.join("codex.cmd");
    fs::write(&shim, "").unwrap();

    let found = find_in_directories(&["codex.cmd".into()], package_npm_directories(&local));
    fs::remove_dir_all(local).unwrap();
    assert_eq!(found, Some(shim));
}

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
