#[cfg(windows)]
use super::resolve_executable;
#[cfg(windows)]
use std::{env, process::Stdio};
use std::{ffi::OsString, path::Path, time::Duration};
use tokio::{process::{Child, Command}, time::timeout};

pub fn searched_locations(name: &str, explicit_path_was_set: bool) -> String {
    let mut locations = Vec::new();
    if explicit_path_was_set {
        locations.push("configured file path");
    }
    locations.push("process PATH");
    #[cfg(windows)]
    {
        locations.push("user/machine registry PATH");
        locations.push("%APPDATA%\\npm");
        locations.push("%LOCALAPPDATA%\\Programs\\nodejs");
        locations.push("%USERPROFILE%\\.local\\bin");
        if name.eq_ignore_ascii_case("codex") {
            locations.push("%LOCALAPPDATA%\\OpenAI\\Codex\\bin\\*");
        }
    }
    #[cfg(not(windows))]
    let _ = name;
    locations.join(", ")
}

pub fn command_for(path: &Path, args: &[OsString]) -> Command {
    let is_cmd = path
        .extension()
        .is_some_and(|extension| extension.to_string_lossy().eq_ignore_ascii_case("cmd"));
    let mut command = if cfg!(windows) && is_cmd {
        // Keep user values out of cmd.exe's command text. Each argument is placed in
        // the child environment and expanded inside its own quoted argument.
        let mut command = Command::new("cmd");
        command.args(["/D", "/S", "/C"]);
        let mut command_line = String::from("\"\"%AI_HUB_CLI_EXECUTABLE%\"");
        for (index, _) in args.iter().enumerate() {
            command_line.push_str(&format!(" \"%AI_HUB_CLI_ARG_{index}%\""));
        }
        command_line.push('"');
        append_cmd_line(&mut command, &command_line);
        command.env("AI_HUB_CLI_EXECUTABLE", path);
        for (index, argument) in args.iter().enumerate() {
            command.env(format!("AI_HUB_CLI_ARG_{index}"), argument);
        }
        command
    } else {
        let mut command = Command::new(path);
        command.args(args);
        command
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.as_std_mut().creation_flags(CREATE_NO_WINDOW);

        if is_cmd {
            let mut prepend = Vec::new();
            if let Some(parent) = path.parent() {
                prepend.push(parent.to_path_buf());
            }
            if let Some(node) = resolve_executable("node", None).filter(|node| {
                node.extension().is_some_and(|extension| {
                    extension.to_string_lossy().eq_ignore_ascii_case("exe")
                })
            }) {
                if let Some(parent) = node.parent() {
                    prepend.push(parent.to_path_buf());
                }
            }
            if let Some(existing) = env::var_os("PATH") {
                prepend.extend(env::split_paths(&existing));
            }
            if let Ok(path) = env::join_paths(prepend) {
                command.env("PATH", path);
            }
        }
    }
    command
}

#[cfg(windows)]
fn append_cmd_line(command: &mut Command, command_line: &str) {
    use std::os::windows::process::CommandExt;
    command.as_std_mut().raw_arg(command_line);
}

#[cfg(not(windows))]
fn append_cmd_line(command: &mut Command, command_line: &str) {
    command.arg(command_line);
}

pub(crate) async fn terminate_child_tree(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.wait().await;
        return;
    }
    #[cfg(windows)]
    {
        if let Some(pid) = child.id() {
            let mut taskkill = Command::new("taskkill");
            taskkill
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            taskkill.as_std_mut().creation_flags(CREATE_NO_WINDOW);
            if !matches!(taskkill.status().await, Ok(status) if status.success()) {
                let _ = child.kill().await;
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = child.kill().await;
    }
    let _ = timeout(Duration::from_secs(3), child.wait()).await;
}
