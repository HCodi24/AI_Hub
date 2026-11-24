use std::{path::Path, process::Stdio, time::Instant};
use tokio::{process::Command, time::timeout};

/// Run a user-written command line through `cmd.exe` exactly as typed.
/// `Command::args` would re-quote it (turning `"C:\a b\x.py"` into `\"C:\a b\x.py\"`, which cmd does not
/// understand), so pass it raw: `cmd /D /S /C "<command>"` — with /S, cmd strips only the outer quotes.
#[cfg(windows)]
fn windows_shell_command(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut c = Command::new("cmd");
    c.args(["/D", "/S", "/C"]);
    c.as_std_mut().raw_arg(format!("\"{command}\""));
    c
}

#[cfg(not(windows))]
fn windows_shell_command(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.args(["-lc", command]);
    c
}

pub(super) async fn run_command(
    folder: &Path,
    command: &str,
    deadline: Instant,
    tx: &tokio::sync::mpsc::Sender<String>,
) -> (bool, String) {
    let mut cmd = if cfg!(windows) {
        windows_shell_command(command)
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
            crate::core::process::kill_process_tree(pid).await;
            return (false, "Đã hủy kiểm tra".into());
        },
    };
    match output {
        Ok(Ok(Ok(o))) => {
            let s = crate::core::text::strip_ansi(&format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ));
            (o.status.success(), s)
        }
        Ok(Ok(Err(e))) => (false, e.to_string()),
        Ok(Err(e)) => (false, e.to_string()),
        Err(_) => {
            crate::core::process::kill_process_tree(pid).await;
            (false, "Quá thời gian 15 phút".into())
        }
    }
}
