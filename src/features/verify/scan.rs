use crate::core::process::git_output;
use std::path::{Path, PathBuf};

pub(super) async fn changed_files(folder: &Path) -> Vec<PathBuf> {
    if let Ok(o) = git_output(folder, &["diff", "--name-only", "HEAD"]).await {
        if o.status.success() {
            let mut files = String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| folder.join(s))
                .collect::<Vec<_>>();
            if let Ok(output) =
                git_output(folder, &["ls-files", "--others", "--exclude-standard"]).await
            {
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
    for top in ["src", "web/src", "app"] {
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
pub(super) fn scan_invalid_utf8(folder: &Path, files: &[PathBuf]) -> Vec<String> {
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
pub(super) fn scan_mojibake(folder: &Path, files: &[PathBuf], ignore: &[String]) -> Vec<String> {
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
pub(super) fn question_pattern(line: &str) -> bool {
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
