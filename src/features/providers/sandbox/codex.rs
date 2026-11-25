use std::path::Path;

#[cfg(not(windows))]
pub(crate) fn share_sandbox_state(_: &Path, _: &Path) {}

#[cfg(windows)]
pub(crate) use windows::share_sandbox_state;

#[cfg(windows)]
mod windows {
    use super::Path;
    use crate::{core::time::now_ms, providers::cli_support::valid_provider_id};
    use std::{
        fs, io,
        os::windows::{fs::MetadataExt, process::CommandExt},
        path::PathBuf,
        process::{Command, Stdio},
        sync::Mutex,
    };

    const NAMES: [&str; 2] = [".sandbox-secrets", ".sandbox"];
    const MARKER: &str = ".sandbox/setup_marker.json";
    const REPARSE_POINT: u32 = 0x400;
    static MIGRATION: Mutex<()> = Mutex::new(());

    // Codex homes provision the same Windows users. Sharing their setup files
    // avoids invalidating credentials and prompting for UAC on every switch.
    pub(crate) fn share_sandbox_state(
        codex_root: &Path,
        home: &Path,
    ) {
        let _guard = MIGRATION.lock().unwrap_or_else(|e| e.into_inner());
        let shared = codex_root.join(".shared-sandbox");
        if NAMES
            .iter()
            .all(|name| linked(&home.join(name), &shared.join(name)))
        {
            return;
        }

        let mut moves = Vec::new();
        let mut junctions = Vec::new();
        if migrate(codex_root, home, &shared, &mut moves, &mut junctions).is_err() {
            for junction in junctions.iter().rev() {
                // Only remove junctions created by this attempt. remove_dir does
                // not follow directory junctions or remove their contents.
                let _ = fs::remove_dir(junction);
            }
            for (original, moved) in moves.iter().rev() {
                if fs::symlink_metadata(original)
                    .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
                {
                    let _ = fs::rename(moved, original);
                }
            }
        }
    }

    fn migrate(
        root: &Path,
        home: &Path,
        shared: &Path,
        moves: &mut Vec<(PathBuf, PathBuf)>,
        junctions: &mut Vec<PathBuf>,
    ) -> io::Result<()> {
        fs::create_dir_all(shared)?;
        let seed = if shared.join(MARKER).try_exists()? {
            None
        } else {
            newest_home(root)?
        };
        if let Some(seed) = &seed {
            for name in NAMES {
                let target = shared.join(name);
                backup(&target, moves)?;
                move_dir(seed.join(name), target, moves)?;
            }
        }

        let mut pending = Vec::new();
        for name in NAMES {
            let target = shared.join(name);
            fs::create_dir_all(&target)?;
            // Keep the donor usable immediately after moving its state.
            for directory in
                std::iter::once(home).chain(seed.as_deref().filter(|path| *path != home))
            {
                let entry = directory.join(name);
                if !linked(&entry, &target) {
                    backup(&entry, moves)?;
                    pending.push((entry, target.clone()));
                }
            }
        }
        for (entry, target) in pending {
            junction(&entry, &target)?;
            junctions.push(entry);
        }
        Ok(())
    }

    fn newest_home(root: &Path) -> io::Result<Option<PathBuf>> {
        Ok(fs::read_dir(root)?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                if !entry.file_type().ok()?.is_dir()
                    || !valid_provider_id(entry.file_name().to_str()?)
                {
                    return None;
                }
                let path = entry.path();
                let modified = fs::metadata(path.join(MARKER)).ok()?.modified().ok()?;
                Some((modified, path))
            })
            .max()
            .map(|(_, path)| path))
    }

    fn linked(entry: &Path, target: &Path) -> bool {
        matches!(
            (fs::canonicalize(entry), fs::canonicalize(target)),
            (Ok(entry), Ok(target)) if entry == target
        )
    }

    fn backup(path: &Path, moves: &mut Vec<(PathBuf, PathBuf)>) -> io::Result<()> {
        match fs::symlink_metadata(path) {
            Ok(_) => {
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let backup = path.with_file_name(format!("{name}.bak-{}", now_ms()));
                move_dir(path.to_owned(), backup, moves)
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    fn move_dir(from: PathBuf, to: PathBuf, moves: &mut Vec<(PathBuf, PathBuf)>) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&from)?;
        if !metadata.is_dir() || metadata.file_attributes() & REPARSE_POINT != 0 {
            return Err(io::Error::other("expected a real sandbox directory"));
        }
        match fs::symlink_metadata(&to) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
            Ok(_) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
        }
        fs::rename(&from, &to)?;
        moves.push((from, to));
        Ok(())
    }

    fn junction(entry: &Path, target: &Path) -> io::Result<()> {
        // Expand each quoted path once so cmd metacharacters in paths stay literal.
        let status = Command::new("cmd.exe")
            .args(["/D", "/V:OFF", "/C", "mklink", "/J"])
            .raw_arg(r#""%AI_HUB_JUNCTION_LINK%""#)
            .raw_arg(r#""%AI_HUB_JUNCTION_TARGET%""#)
            .env("AI_HUB_JUNCTION_LINK", std::path::absolute(entry)?)
            .env("AI_HUB_JUNCTION_TARGET", std::path::absolute(target)?)
            .creation_flags(0x08000000)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()?;
        if status.success() {
            Ok(())
        } else {
            Err(io::Error::other("mklink failed"))
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::share_sandbox_state;
    use std::{
        fs, io,
        os::windows::fs::MetadataExt,
        path::{Path, PathBuf},
        thread,
        time::Duration,
    };

    const STATE_FILES: [(&str, &str); 2] = [
        (".sandbox", "setup_marker.json"),
        (".sandbox-secrets", "sandbox_users.json"),
    ];

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn cleanup(&self) -> io::Result<()> {
            for home in ["a", "b"] {
                for (directory, _) in STATE_FILES {
                    let path = self.0.join(home).join(directory);
                    match fs::symlink_metadata(&path) {
                        Ok(metadata) if metadata.file_attributes() & 0x400 != 0 => {
                            fs::remove_dir(path)?;
                        }
                        Ok(_) => {}
                        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                        Err(error) => return Err(error),
                    }
                }
            }
            match fs::remove_dir_all(&self.0) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = self.cleanup();
        }
    }

    fn write_home(home: &Path, contents: &str) {
        for (directory, file) in STATE_FILES.into_iter().chain([(".sandbox-bin", "x")]) {
            let path = home.join(directory);
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join(file), contents).unwrap();
        }
    }

    fn backups(home: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(home)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.contains(".bak-"))
            .collect();
        names.sort();
        names
    }

    #[test]
    fn shares_the_newest_sandbox_pair_and_preserves_local_data() {
        let root = TempRoot(std::env::temp_dir().join(uuid::Uuid::new_v4().to_string()));
        let a = root.0.join("a");
        let b = root.0.join("b");
        write_home(&a, "a");
        thread::sleep(Duration::from_millis(50));
        write_home(&b, "b");
        let marker_time = |home: &Path| {
            fs::metadata(home.join(".sandbox/setup_marker.json"))
                .unwrap()
                .modified()
                .unwrap()
        };
        assert!(marker_time(&b) > marker_time(&a));

        share_sandbox_state(&root.0, &a);
        share_sandbox_state(&root.0, &b);

        let shared = root.0.join(".shared-sandbox");
        let original_backups = backups(&a);
        assert_eq!(original_backups.len(), STATE_FILES.len());
        for (directory, file) in STATE_FILES {
            let shared_directory = shared.join(directory);
            assert_eq!(fs::read_to_string(shared_directory.join(file)).unwrap(), "b");
            let target = fs::canonicalize(shared_directory).unwrap();
            for home in [&a, &b] {
                assert_eq!(fs::canonicalize(home.join(directory)).unwrap(), target);
            }
            let prefix = format!("{directory}.bak-");
            let backup = original_backups
                .iter()
                .find(|name| name.starts_with(&prefix))
                .unwrap();
            assert_eq!(fs::read_to_string(a.join(backup).join(file)).unwrap(), "a");
        }
        for home in [&a, &b] {
            assert!(home.join(".sandbox-bin/x").is_file());
        }
        fs::write(a.join(".sandbox/shared.txt"), "shared").unwrap();
        assert_eq!(
            fs::read_to_string(b.join(".sandbox/shared.txt")).unwrap(),
            "shared"
        );

        share_sandbox_state(&root.0, &a);
        assert_eq!(backups(&a), original_backups);

        root.cleanup().unwrap();
    }
}
