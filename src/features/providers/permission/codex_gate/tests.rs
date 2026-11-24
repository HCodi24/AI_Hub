use super::{decision, writes, Gate, Verdict};
use serde_json::json;
use std::path::Path;

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
fn write_commands_are_recognized() {
    for command in [r#"powershell -Command "Set-Content -LiteralPath 'a.txt' -Value 'hi'""#, "echo hi > a.txt", "rm -rf x", "git commit -m x", "npm install",
        "[System.IO.File]::WriteAllText('a','hi')", "cargo build", "python -c \"open('a','w')\"", "Invoke-WebRequest x -OutFile a", "git push", "Get-ChildItem | Foo-Bar"] {
        assert!(writes(command), "{command}");
    }
    for command in ["Get-ChildItem src -Name", "cat src/main.rs", "git status", "rg foo src 2>&1", "git log -n 3",
        r#""C:\WINDOWS\System32\WindowsPowerShell1.0\powershell.exe" -Command "Get-ChildItem -Force | Select-Object -First 3""#] {
        assert!(!writes(command), "{command}");
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
