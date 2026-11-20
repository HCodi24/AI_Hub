//! Approval gate for Codex commands. Codex's Windows sandbox cannot limit reads to a folder, so the app runs Codex with
//! `approvalPolicy: untrusted` and judges every command Codex asks to run: a path outside the allowed folders is declined.
//! shortcut: it reads paths out of the command text (absolute paths, `..` and home-folder names), so a command that builds
//! a path at run time gets through; upgrade to a real sandbox read limit when Codex supports one on Windows.
use serde_json::{json, Value};
use std::path::Path;

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
mod tests {
    use super::*;

    fn gate(users: &[&str]) -> Gate {
        Gate::new(Path::new("C:\\GitHub\\AI_Hub"), users.iter().copied())
    }
    fn denied(gate: &Gate, command: &str) -> Vec<String> {
        match gate.judge("item/commandExecution/requestApproval", &json!({ "command": command })) {
            Some(Verdict::Deny(paths)) => paths,
            Some(Verdict::Allow) => Vec::new(),
            None => panic!("not an approval"),
        }
    }

    #[test]
    fn commands_inside_the_project_folder_pass() {
        let g = gate(&[]);
        for command in [
            r#""C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe" -Command 'git status --short'"#,
            r#"powershell -Command "Get-Content C:\GitHub\AI_Hub\src\main.rs""#,
            r#"powershell -Command "Get-Content c:/github/ai_hub/Cargo.toml""#,
            "cargo test --quiet",
            r#"Get-ChildItem src\providers -Name; Get-Content ..\AI_Hub\Cargo.toml"#,
        ] {
            assert!(denied(&g, command).is_empty(), "{command}");
        }
    }

    #[test]
    fn regex_escapes_in_a_command_are_not_paths() {
        let g = gate(&[]);
        for command in [
            r#"Get-ChildItem src/db/migration | ForEach-Object { if ($_.Name -match '^V(\\d+)__') { [int]$Matches[1] } }"#,
            r#""C:\WINDOWS\System32\WindowsPowerShell\v1.0\powershell.exe" -Command "rg -n 'Yêu cầu|\\.desktopHeader' src""#,
        ] {
            assert!(denied(&g, command).is_empty(), "{command}");
        }
    }

    #[test]
    fn a_path_outside_the_folder_is_declined() {
        let g = gate(&[]);
        assert_eq!(denied(&g, r#"powershell -Command "Get-ChildItem C:\GitHub\BEANS -Name""#), vec![r"C:\GitHub\BEANS"]);
        assert_eq!(denied(&g, r#"cat 'C:/Users/me/Documents/a.txt'"#).len(), 1);
        // A sibling whose name starts with the project name is still outside.
        assert_eq!(denied(&g, r"dir C:\GitHub\AI_Hub2\src").len(), 1);
        assert_eq!(denied(&g, r"type \\server\share\a.txt").len(), 1);
    }

    #[test]
    fn climbing_out_of_the_folder_and_the_home_folder_are_declined() {
        let g = gate(&[]);
        assert_eq!(denied(&g, r"Get-ChildItem ..\BEANS -Name"), vec![r"..\BEANS"]);
        assert_eq!(denied(&g, r#"Get-Content "../../x.txt""#).len(), 1);
        assert!(denied(&g, r"Get-ChildItem src\..\web -Name").is_empty());
        assert_eq!(denied(&g, r"Get-ChildItem ~\Documents").len(), 1);
        assert_eq!(denied(&g, r"Get-ChildItem $env:USERPROFILE\Desktop").len(), 1);
    }

    #[test]
    fn a_folder_the_user_named_is_allowed() {
        let g = gate(&[r"đọc thử C:\GitHub\BEANS\SCM-BEANS_Dashboard_FE giúp tôi", r"xem C:\Docs\note.md"]);
        assert!(denied(&g, r"Get-ChildItem C:\GitHub\BEANS\SCM-BEANS_Dashboard_FE\src -Name").is_empty());
        assert!(denied(&g, r"Get-Content C:\Docs\other.md").is_empty());
        assert_eq!(denied(&g, r"Get-ChildItem C:\GitHub\BEANS\mein-idaas").len(), 1);
    }

    /// Real Codex with `approvalPolicy: untrusted`: a command that reads another repository must reach the gate and be declined,
    /// and one inside the folder must run. Uses the app's own Codex home (one short, low-effort turn on its account).
    #[tokio::test]
    #[ignore = "Needs MUTIS_CODEX_TEST_EXE and the app's signed-in Codex home; runs one real turn"]
    async fn live_codex_asks_the_gate_before_reading_another_repository() {
        use crate::providers::codex::transport::Client;
        let exe = std::env::var_os("MUTIS_CODEX_TEST_EXE").expect("Set MUTIS_CODEX_TEST_EXE");
        let home = std::path::PathBuf::from(std::env::var("APPDATA").unwrap()).join("ai-hub").join("codex").join("codex-cli");
        let folder = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let gate = Gate::new(&folder, std::iter::empty());
        let mut client = Client::start(Path::new(&exe), &home, &folder).await.unwrap();
        let thread = client.call("thread/start", json!({"cwd":folder,"model":"gpt-6.2-sol","approvalPolicy":"untrusted","sandbox":"workspace-write","selectedCapabilityRoots":[]})).await.unwrap();
        let id = thread["thread"]["id"].as_str().unwrap().to_owned();
        let prompt = "Run these two PowerShell commands one after the other and then reply only DONE: (1) Get-ChildItem C:\\GitHub\\BEANS -Name  (2) Get-ChildItem src -Name";
        client.call("turn/start", json!({"threadId":id,"effort":"low","input":[{"type":"text","text":prompt}]})).await.unwrap();
        let (mut allowed, mut declined) = (Vec::new(), Vec::new());
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(180);
        loop {
            let value = tokio::time::timeout_at(deadline, client.next()).await.expect("turn took too long").unwrap();
            if value.get("id").is_some() && value.get("method").is_some() {
                let method = value["method"].as_str().unwrap_or("");
                match gate.judge(method, &value["params"]) {
                    Some(verdict) => {
                        let command = value["params"]["command"].as_str().unwrap_or("").to_owned();
                        if matches!(verdict, Verdict::Allow) { allowed.push(command) } else { declined.push(command) }
                        client.respond(&value, decision(&verdict)).await.unwrap();
                    }
                    None => client.reject_request(&value).await.unwrap(),
                }
            } else if value["method"] == "turn/completed" {
                break;
            }
        }
        client.close().await;
        assert!(declined.iter().any(|c| c.contains("BEANS")), "BEANS command was never declined: allowed={allowed:?} declined={declined:?}");
        assert!(!allowed.iter().any(|c| c.contains("BEANS")), "BEANS command was allowed: {allowed:?}");
    }

    #[test]
    fn edits_pass_and_other_requests_are_left_to_the_caller() {
        let g = gate(&[]);
        assert!(matches!(g.judge("item/fileChange/requestApproval", &json!({})), Some(Verdict::Allow)));
        assert!(g.judge("item/permissions/requestApproval", &json!({})).is_none());
        assert_eq!(decision(&Verdict::Allow)["decision"], "accept");
        assert_eq!(decision(&Verdict::Deny(vec![]))["decision"], "decline");
    }
}
