//! Approval gate for Codex commands. Codex's Windows sandbox cannot limit reads to a folder, so the app runs Codex with
//! `approvalPolicy: untrusted` and judges every command Codex asks to run: a path outside the allowed folders is declined.
//! shortcut: it reads paths out of the command text (absolute paths, `..` and home-folder names), so a command that builds
//! a path at run time gets through; upgrade to a real sandbox read limit when Codex supports one on Windows.
use serde_json::{json, Value};
use std::path::Path;

mod writes;

pub(crate) use writes::writes;

pub(crate) enum Verdict {
    Allow,
    Deny(Vec<String>),
}

pub(crate) struct Gate {
    cwd: Vec<String>,
    roots: Vec<Vec<String>>,
}

/// Lower-cased path parts with `.` and `..` resolved; None when the text is not an absolute path.
fn parts(path: &str) -> Option<Vec<String>> {
    let path = path.replace('/', "\\");
    let path = path.trim_end_matches('\\');
    let bytes = path.as_bytes();
    let absolute = (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':') || path.starts_with("\\\\");
    if !absolute {
        return None;
    }
    let mut out: Vec<String> = Vec::new();
    for part in path.split('\\').filter(|p| !p.is_empty()) {
        match part {
            "." => {}
            ".." => { out.pop(); }
            _ => out.push(part.to_lowercase()),
        }
    }
    Some(out)
}

fn inside(path: &[String], root: &[String]) -> bool {
    path.len() >= root.len() && path[..root.len()] == *root
}

fn is_stop(c: char) -> bool {
    c.is_whitespace() || "\"'`|;&<>()[]{},".contains(c)
}

/// `\\host\share...` at `i`. A regex escape such as `\\d+` or `|\\.name` has no `host\share` shape, so it is not a path.
fn is_unc(chars: &[char], i: usize) -> bool {
    let host_end = (i + 2..chars.len()).find(|&j| is_stop(chars[j]) || matches!(chars[j], '\\' | '/')).unwrap_or(chars.len());
    host_end > i + 2 && matches!(chars.get(host_end), Some('\\' | '/')) && chars.get(host_end + 1).is_some_and(|c| !is_stop(*c))
}

/// Absolute paths written in a text (drive letters and UNC).
fn absolute_paths(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let drive = chars[i].is_ascii_alphabetic() && chars.get(i + 1) == Some(&':') && matches!(chars.get(i + 2), Some('\\' | '/'))
            && (i == 0 || !chars[i - 1].is_ascii_alphanumeric());
        let unc = chars[i] == '\\' && chars.get(i + 1) == Some(&'\\') && (i == 0 || is_stop(chars[i - 1])) && is_unc(&chars, i);
        if drive || unc {
            let end = (i..chars.len()).find(|&j| is_stop(chars[j])).unwrap_or(chars.len());
            found.push(chars[i..end].iter().collect::<String>());
            i = end;
        } else {
            i += 1;
        }
    }
    found
}

impl Gate {
    /// Allowed: the chat folder, the Windows and program folders the tools live in, and every folder the user wrote in the chat.
    pub(crate) fn new<'a>(folder: &Path, user_texts: impl Iterator<Item = &'a str>) -> Self {
        let folder_text = crate::core::text::strip_verbatim(folder.to_path_buf()).to_string_lossy().into_owned();
        let cwd = parts(&folder_text).unwrap_or_default();
        let mut roots = vec![cwd.clone()];
        for name in ["SystemRoot", "ProgramFiles", "ProgramFiles(x86)", "ProgramData", "TEMP"] {
            if let Some(root) = std::env::var(name).ok().and_then(|v| parts(&v)) {
                roots.push(root);
            }
        }
        for text in user_texts {
            for path in absolute_paths(text) {
                if let Some(mut root) = parts(&path) {
                    // A file the user named allows its folder.
                    if Path::new(&path).extension().is_some() { root.pop(); }
                    if !root.is_empty() { roots.push(root); }
                }
            }
        }
        Self { cwd, roots }
    }

    /// Allow reading one more folder (the enabled plugins' skill files).
    pub(crate) fn allow(&mut self, folder: &Path) {
        if let Some(root) = parts(&crate::core::text::strip_verbatim(folder.to_path_buf()).to_string_lossy()) { self.roots.push(root); }
    }

    /// Paths and names in the command that point outside the allowed folders.
    fn outside(&self, command: &str) -> Vec<String> {
        let mut out = Vec::new();
        for path in absolute_paths(command) {
            if let Some(p) = parts(&path) {
                if !self.roots.iter().any(|root| inside(&p, root)) { out.push(path); }
            }
        }
        let project_in_home = std::env::var("USERPROFILE").ok().and_then(|v| parts(&v)).is_some_and(|home| inside(&self.cwd, &home));
        for token in command.split(is_stop) {
            let token = token.trim_matches(|c| c == '\\' || c == '/');
            let home = ["~", "$home", "$env:userprofile", "%userprofile%", "$env:homepath"].iter().any(|h| token.to_lowercase().starts_with(h));
            let climbs = token.replace('/', "\\").split('\\').any(|p| p == "..");
            if home && !project_in_home {
                out.push(token.to_owned());
            } else if climbs && !token.contains(':') {
                // A relative path that climbs out of the folder: resolve it against the chat folder.
                let joined = format!("{}\\{}", self.cwd.join("\\"), token.replace('/', "\\"));
                if let Some(p) = parts(&joined) {
                    if !self.roots.iter().any(|root| inside(&p, root)) { out.push(token.to_owned()); }
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// The decision for one request Codex sent to the app. None = not an approval request: the caller rejects it as before.
    pub(crate) fn judge(&self, method: &str, params: &Value) -> Option<Verdict> {
        match method {
            // File edits are already limited to the workspace by the sandbox.
            "item/fileChange/requestApproval" => Some(Verdict::Allow),
            "item/commandExecution/requestApproval" => {
                let outside = self.outside(params["command"].as_str().unwrap_or(""));
                Some(if outside.is_empty() { Verdict::Allow } else { Verdict::Deny(outside) })
            }
            _ => None,
        }
    }
}

pub(crate) fn decision(verdict: &Verdict) -> Value {
    json!({ "decision": if matches!(verdict, Verdict::Allow) { "accept" } else { "decline" } })
}

#[cfg(test)]
mod tests;
