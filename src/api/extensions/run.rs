use super::backup::backup_file;
use crate::providers;
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncBufReadExt, sync::mpsc};
use tokio_util::sync::CancellationToken;

pub(super) struct RunAction {
    pub(super) token: CancellationToken,
    pub(super) exe: Option<PathBuf>,
    pub(super) args: Vec<OsString>,
    pub(super) folder: Option<PathBuf>,
    pub(super) backup: Option<String>,
    pub(super) configured_backups: Option<PathBuf>,
    pub(super) modify: String,
    pub(super) command: &'static str,
}

pub(super) async fn run_action(
    tx: &mpsc::Sender<Result<String, String>>,
    action: RunAction,
) -> Result<(), String> {
    let RunAction {
        token,
        exe,
        args,
        folder,
        backup,
        configured_backups,
        modify,
        command,
    } = action;
    backup_file(tx, backup.as_deref(), configured_backups).await?;
    let path = exe.ok_or_else(|| match command {
        "npm" => "Chưa có Node.js/npm trên máy. Cài Node.js LTS tại https://nodejs.org rồi bấm Cài đặt lại."
            .to_owned(),
        _ => format!("Không tìm thấy executable: {command}"),
    })?;
    let _ = tx
        .send(Ok(format!(
            "{} {} | có thể sửa: {}",
            path.display(),
            args.iter()
                .map(|argument| argument.to_string_lossy())
                .collect::<Vec<_>>()
                .join(" "),
            modify
        )))
        .await;
    let mut cmd = providers::cli_support::command_for(&path, &args);
    if let Some(cwd) = folder.as_ref() {
        cmd.current_dir(cwd);
    }
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = cmd.spawn().map_err(|error| error.to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Không đọc được output".to_string())?;
    let stderr = child.stderr.take();
    if let Some(stderr) = stderr {
        let tx_err = tx.clone();
        tokio::spawn(async move {
            let mut reader = tokio::io::BufReader::new(stderr);
            let mut line = String::new();
            loop {
                line.clear();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                let _ = tx_err.send(Ok(crate::core::text::strip_ansi(&line))).await;
            }
        });
    }
    let mut reader = tokio::io::BufReader::new(stdout);
    let mut line = String::new();
    let wait = async {
        let deadline = tokio::time::sleep(Duration::from_secs(600));
        tokio::pin!(deadline);
        loop {
            line.clear();
            tokio::select! {
                _ = token.cancelled() => {
                    providers::cli_support::terminate_child_tree(&mut child).await;
                    return Err("Đã dừng".to_string());
                }
                _ = &mut deadline => {
                    providers::cli_support::terminate_child_tree(&mut child).await;
                    return Err("Quá thời gian chờ 10 phút".to_string());
                }
                read = reader.read_line(&mut line) => {
                    let n = read.map_err(|error| error.to_string())?;
                    if n == 0 {
                        break;
                    }
                    let _ = tx
                        .send(Ok(crate::core::text::strip_ansi(&line)))
                        .await;
                }
            }
        }
        let status = child.wait().await.map_err(|error| error.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("Lệnh kết thúc với {status}"))
        }
    }
    .await;
    wait
}
