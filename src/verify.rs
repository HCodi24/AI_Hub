use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Instant,
};
use tokio::{
    process::Command,
    time::{timeout, Duration},
};

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResultReport {
    pub ok: bool,
    pub summary_lines: Vec<String>,
    pub checks: Vec<Check>,
    pub duration_ms: u64,
}

pub async fn run(
    folder: &Path,
    custom: &str,
    ignore: &[String],
    tx: tokio::sync::mpsc::Sender<String>,
) -> ResultReport {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(900);
    let mut checks = Vec::new();
    let commands = if !custom.trim().is_empty() {
        vec![("verify_command", custom.to_owned())]
    } else {
        detect(folder)
    };
    if commands.is_empty() {
        let _ = tx
            .send("Không có lệnh kiểm tra cho thư mục này".into())
            .await;
        checks.push(Check {
            name: "verify".into(),
            ok: true,
            detail: "Không có lệnh kiểm tra cho thư mục này".into(),
        });
    }
    for (name, command) in commands {
        let _ = tx.send(format!("Chạy: {command}")).await;
        let (ok, output) = run_command(folder, &command, deadline, &tx).await;
        for line in output
            .lines()
            .rev()
            .take(120)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let _ = tx.send(line.to_owned()).await;
        }
        checks.push(Check {
            name: name.into(),
            ok,
            detail: tail(&output, 4000),
        });
    }
    let files = changed_files(folder).await;
    let invalid_utf8 = scan_invalid_utf8(folder, &files);
    let utf8_ok = invalid_utf8.is_empty();
    let utf8_detail = if utf8_ok {
        "Tất cả tệp văn bản hợp lệ UTF-8".to_owned()
    } else {
        invalid_utf8.join(", ")
    };
    let _ = tx.send(format!("UTF8: {utf8_detail}")).await;
    checks.push(Check {
        name: "UTF8".into(),
        ok: utf8_ok,
        detail: utf8_detail,
    });
    let mojibake = scan_mojibake(folder, &files, ignore);
    let ok = mojibake.is_empty();
    let detail = if ok {
        "Không phát hiện mojibake".to_owned()
    } else {
        mojibake
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    };
    let _ = tx.send(format!("MOJIBAKE: {detail}")).await;
    checks.push(Check {
        name: "MOJIBAKE".into(),
        ok,
        detail,
    });
    let ok = checks.iter().all(|c| c.ok);
    let mut summary_lines = checks
        .iter()
        .map(|c| {
            format!(
                "{}: {} — {}",
                c.name,
                if c.ok { "ĐẠT" } else { "LỖI" },
                c.detail.lines().last().unwrap_or("")
            )
        })
        .take(12)
        .collect::<Vec<_>>();
    if summary_lines.is_empty() {
        summary_lines.push("Không có kết quả kiểm tra".into());
    }
    ResultReport {
        ok,
        summary_lines,
        checks,
        duration_ms: started.elapsed().as_millis() as u64,
    }
}

fn detect(folder: &Path) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let script = folder.join("scripts/verify/verify.sh");
    if script.is_file() && command_available("bash") {
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
fn command_available(name: &str) -> bool {
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
async fn run_command(
    folder: &Path,
    command: &str,
    deadline: Instant,
    tx: &tokio::sync::mpsc::Sender<String>,
) -> (bool, String) {
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.args(["/C", command]);
        c
    } else {
        let mut c = Command::new("sh");
        c.args(["-lc", command]);
        c
    };
    cmd.current_dir(folder)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.as_std_mut().creation_flags(CREATE_NO_WINDOW);
    }
    let child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => return (false, error.to_string()),
    };
    let pid = child.id();
    let output_task = tokio::spawn(async move { child.wait_with_output().await });
    let output = tokio::select! {
        result = timeout(deadline.saturating_duration_since(Instant::now()), output_task) => result,
        _ = tx.closed() => {
            kill_process_tree(pid).await;
            return (false, "Đã hủy kiểm tra".into());
        },
    };
    match output {
        Ok(Ok(Ok(o))) => {
            let s = strip_ansi(&format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
            (o.status.success(), s)
        }
        Ok(Ok(Err(e))) => (false, e.to_string()),
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => {
            kill_process_tree(pid).await;
            (false, "Quá thời gian 15 phút".into())
        }
    }
}
async fn kill_process_tree(pid: Option<u32>) {
    let Some(pid) = pid else { return };
    #[cfg(windows)]
    {
        let mut killer = Command::new("taskkill");
        killer
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        killer.as_std_mut().creation_flags(CREATE_NO_WINDOW);
        let _ = killer.status().await;
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }
}
async fn changed_files(folder: &Path) -> Vec<PathBuf> {
    let mut cmd = Command::new("git");
    cmd.args(["diff", "--name-only", "HEAD"])
        .current_dir(folder)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if let Ok(o) = cmd.output().await {
        if o.status.success() {
            let mut files = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| folder.join(s))
                .collect::<Vec<_>>();
            let mut untracked = Command::new("git");
            untracked
                .args(["ls-files", "--others", "--exclude-standard"])
                .current_dir(folder)
                .stdout(Stdio::piped())
                .stderr(Stdio::null());
            if let Ok(output) = untracked.output().await {
                if output.status.success() {
                    files.extend(
                        String::from_utf8_lossy(&output.stdout)
                            .lines()
                            .map(|s| folder.join(s)),
                    );
                }
            }
            files.sort();
            files.dedup();
            files.truncate(500);
            return files;
        }
    }
    let mut files = walk_text(folder);
    files.truncate(500);
    files
}
fn walk_text(folder: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for top in ["src", "ui", "app"] {
        let root = folder.join(top);
        if root.exists() {
            let mut stack = vec![root];
            while let Some(dir) = stack.pop() {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            stack.push(p)
                        } else if p.metadata().is_ok_and(|m| m.len() < 1_000_000) {
                            out.push(p)
                        }
                    }
                }
            }
        }
    }
    out
}
fn scan_invalid_utf8(folder: &Path, files: &[PathBuf]) -> Vec<String> {
    files
        .iter()
        .filter_map(|path| {
            let bytes = std::fs::read(path).ok()?;
            if bytes.len() > 1_000_000 || is_binary(path, &bytes) {
                return None;
            }
            std::str::from_utf8(&bytes).is_err().then(|| {
                path.strip_prefix(folder)
                    .unwrap_or(path)
                    .display()
                    .to_string()
            })
        })
        .take(100)
        .collect()
}
fn scan_mojibake(folder: &Path, files: &[PathBuf], ignore: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    for path in files {
        if let Ok(bytes) = std::fs::read(path) {
            if bytes.len() > 1_000_000
                || is_binary(path, &bytes)
                || std::str::from_utf8(&bytes).is_err()
                || ignored(folder, path, ignore)
            {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            for (i, line) in text.lines().enumerate() {
                if line.contains('\u{fffd}')
                    || line.contains('\u{00c3}')
                    || line.contains(&format!("{}{}", '\u{00e1}', '\u{00bb}'))
                    || line.contains(&format!("{}{}", '\u{00e1}', '\u{00ba}'))
                    || line.contains(&format!("{}{}", '\u{0102}', '\u{0192}'))
                    || line.contains(&format!(
                        "{}{}{}{}",
                        '\u{0102}', '\u{00a1}', '\u{00c2}', '\u{00bb}'
                    ))
                    || line.contains(&format!(
                        "{}{}{}{}",
                        '\u{0102}', '\u{00a1}', '\u{00c2}', '\u{00ba}'
                    ))
                    || question_pattern(line)
                {
                    out.push(format!(
                        "{}:{}",
                        path.strip_prefix(folder).unwrap_or(path).display(),
                        i + 1
                    ));
                    if out.len() >= 100 {
                        return out;
                    }
                }
            }
        }
    }
    out
}
fn is_binary(path: &Path, bytes: &[u8]) -> bool {
    let binary_ext = [
        "png", "jpg", "jpeg", "gif", "webp", "ico", "pdf", "zip", "gz", "exe", "dll", "bin",
        "woff", "woff2", "ttf", "mp3", "mp4",
    ];
    path.extension()
        .and_then(|x| x.to_str())
        .is_some_and(|x| binary_ext.contains(&x.to_ascii_lowercase().as_str()))
        || bytes.contains(&0)
}
fn ignored(folder: &Path, path: &Path, patterns: &[String]) -> bool {
    let relative = path
        .strip_prefix(folder)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/");
    patterns
        .iter()
        .any(|pattern| glob_matches(pattern, &relative))
}
fn glob_matches(pattern: &str, path: &str) -> bool {
    fn go(p: &[u8], s: &[u8]) -> bool {
        if p.is_empty() {
            return s.is_empty();
        }
        if p.starts_with(b"**/") {
            return go(&p[3..], s) || (!s.is_empty() && go(p, &s[1..]));
        }
        if p[0] == b'*' {
            let double = p.get(1) == Some(&b'*');
            let rest = if double { &p[2..] } else { &p[1..] };
            return go(rest, s) || (!s.is_empty() && (double || s[0] != b'/') && go(p, &s[1..]));
        }
        !s.is_empty() && p[0] == s[0] && go(&p[1..], &s[1..])
    }
    go(pattern.as_bytes(), path.as_bytes())
}
fn question_pattern(line: &str) -> bool {
    let chars = line.chars().collect::<Vec<_>>();
    for (i, c) in chars.iter().enumerate() {
        if *c != '?'
            || chars
                .get(i + 1)
                .is_some_and(|next| *next == '.' || *next == '?')
        {
            continue;
        }
        let left = chars[..i]
            .iter()
            .rev()
            .take(4)
            .take_while(|c| c.is_alphabetic())
            .count();
        let right = chars
            .iter()
            .skip(i + 1)
            .take(6)
            .take_while(|c| c.is_lowercase())
            .count();
        if left > 0 && right > 0 {
            return true;
        }
    }
    false
}
#[cfg(test)]
fn may_create_fix(verdict: &str, has_blocker: bool, rounds_used: u32, rounds_max: u32) -> bool {
    verdict == "FAIL" && has_blocker && rounds_used < rounds_max
}
fn tail(s: &str, n: usize) -> String {
    s.chars()
        .rev()
        .take(n)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}
#[cfg(test)]
struct ReviewTaskState {
    verify_ok: bool,
}
#[derive(Debug, PartialEq, Eq)]
#[cfg(test)]
enum ReviewAction {
    Skip,
    Complete,
    Fail,
    CreateFix,
}
#[cfg(test)]
fn next_review_action(
    task: &ReviewTaskState,
    verdict: &str,
    blocking_issue: bool,
    rounds_used: u32,
    max_rounds: u32,
) -> ReviewAction {
    if verdict == "SKIPPED" && !task.verify_ok {
        ReviewAction::Fail
    } else if verdict == "SKIPPED" {
        ReviewAction::Skip
    } else if verdict != "FAIL" {
        ReviewAction::Complete
    } else if blocking_issue && rounds_used < max_rounds {
        ReviewAction::CreateFix
    } else {
        ReviewAction::Fail
    }
}
fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut esc = false;
    for c in s.chars() {
        if c == '\u{1b}' {
            esc = true;
            continue;
        }
        if esc {
            if c.is_ascii_alphabetic() {
                esc = false
            }
            continue;
        }
        out.push(c)
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_detect_prefers_script_over_cargo() {
        let root = std::env::temp_dir().join(format!("ai-hub-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("scripts/verify")).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        std::fs::write(root.join("scripts/verify/verify.sh"), "true").unwrap();
        assert_eq!(detect(&root)[0].0, "verify_script");
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn auto_detect_cargo_commands_are_ordered() {
        let root = std::env::temp_dir().join(format!("ai-hub-cargo-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[package]").unwrap();
        let commands = detect(&root);
        assert_eq!(commands[0].1, "cargo build --offline");
        assert_eq!(commands[1].1, "cargo test --offline");
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn mojibake_question_detector() {
        let broken_word = ["l", "?", "i"].concat();
        assert!(question_pattern(&broken_word));
        assert!(!question_pattern("const x = a?.b ?? c;"));
        assert!(!question_pattern("hello world"));
    }
    #[test]
    fn fix_round_accounting_stops_at_limit() {
        assert!(may_create_fix("FAIL", true, 0, 2));
        assert!(may_create_fix("FAIL", true, 1, 2));
        assert!(!may_create_fix("FAIL", true, 2, 2));
        assert!(!may_create_fix("PASS", true, 0, 2));
        assert!(!may_create_fix("FAIL", false, 0, 2));
    }
    #[test]
    fn review_transition_skips_without_consuming_a_round() {
        assert_eq!(
            next_review_action(&ReviewTaskState { verify_ok: true }, "SKIPPED", true, 0, 2),
            ReviewAction::Skip
        );
        assert_eq!(
            next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", true, 0, 2),
            ReviewAction::CreateFix
        );
        assert_eq!(
            next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", true, 2, 2),
            ReviewAction::Fail
        );
        assert_eq!(
            next_review_action(&ReviewTaskState { verify_ok: true }, "FAIL", false, 0, 2),
            ReviewAction::Fail
        );
        assert_eq!(
            next_review_action(&ReviewTaskState { verify_ok: true }, "PASS", true, 0, 2),
            ReviewAction::Complete
        );
        assert_eq!(
            next_review_action(
                &ReviewTaskState { verify_ok: false },
                "SKIPPED",
                false,
                0,
                2
            ),
            ReviewAction::Fail
        );
    }
    #[test]
    fn invalid_encoding_is_reported_and_ignore_applies_only_to_mojibake() {
        let root = std::env::temp_dir().join(format!("ai-hub-utf8-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".tasks")).unwrap();
        let bad = root.join("hello.txt");
        std::fs::write(&bad, [b'x', 0xe0, b'y']).unwrap();
        let quoted = root.join(".tasks/quoted.md");
        std::fs::write(&quoted, ["l", "?", "i"].concat()).unwrap();
        assert_eq!(
            scan_invalid_utf8(&root, std::slice::from_ref(&bad)),
            vec!["hello.txt"]
        );
        assert!(scan_mojibake(&root, &[quoted], &[".tasks/**".into()]).is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn changed_files_includes_untracked_git_files() {
        let root = std::env::temp_dir().join(format!("ai-hub-untracked-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let _ = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&root)
            .status()
            .await
            .unwrap();
        std::fs::write(root.join("base.txt"), "base").unwrap();
        let _ = Command::new("git")
            .args(["add", "base.txt"])
            .current_dir(&root)
            .status()
            .await
            .unwrap();
        let _ = Command::new("git")
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "base",
                "-q",
            ])
            .current_dir(&root)
            .status()
            .await
            .unwrap();
        let new_file = root.join("hello.txt");
        std::fs::write(&new_file, "xin chào").unwrap();
        assert!(changed_files(&root).await.contains(&new_file));
        std::fs::remove_dir_all(root).unwrap();
    }
}
