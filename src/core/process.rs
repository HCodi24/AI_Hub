use std::{path::Path, process::Output, time::Duration};
use tokio::{process::Command, time::timeout};

pub fn configure_hidden(command: &mut Command) {
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
}

pub async fn run_with_timeout(
    command: &mut Command,
    duration: Duration,
) -> std::io::Result<std::process::Output> {
    match timeout(duration, command.output()).await {
        Ok(result) => result,
        Err(_) => Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "process timed out",
        )),
    }
}

pub async fn git_output(repo: &Path, args: &[&str]) -> std::io::Result<Output> {
    let mut command = Command::new("git");
    command.args(args).current_dir(repo);
    configure_hidden(&mut command);
    run_with_timeout(&mut command, Duration::from_secs(30)).await
}

pub async fn kill_process_tree(pid: Option<u32>) {
    #[cfg(windows)]
    if let Some(pid) = pid {
        let mut command = Command::new("taskkill");
        command.args(["/F", "/T", "/PID", &pid.to_string()]);
        configure_hidden(&mut command);
        let _ = command.status().await;
    }
    #[cfg(not(windows))]
    if let Some(pid) = pid {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
    }
}
