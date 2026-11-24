use crate::core::time::{civil, now_secs, Civil};
use std::path::PathBuf;
use tokio::sync::mpsc;

pub(super) async fn backup_file(
    tx: &mpsc::Sender<Result<String, String>>,
    backup: Option<&str>,
    configured_backups: Option<PathBuf>,
) -> Result<(), String> {
    if let Some(file) = backup {
        if let Some(home) = dirs::home_dir() {
            let source = home.join(file);
            if source.exists() {
                let dir = configured_backups.unwrap_or_else(|| {
                    dirs::config_dir()
                        .unwrap_or_else(|| home.clone())
                        .join("ai-hub")
                        .join("backups")
                });
                tokio::fs::create_dir_all(&dir)
                    .await
                    .map_err(|error| error.to_string())?;
                let stamp = backup_timestamp();
                let target = dir.join(format!(
                    "{}.{}.bak",
                    source.file_name().unwrap_or_default().to_string_lossy(),
                    stamp
                ));
                tokio::fs::copy(&source, &target)
                    .await
                    .map_err(|error| error.to_string())?;
                let _ = tx.send(Ok(format!("Backup: {}", target.display()))).await;
            }
        }
    }
    Ok(())
}

pub(super) fn backup_timestamp() -> String {
    format_backup_timestamp(now_secs())
}

pub(super) fn format_backup_timestamp(secs: u64) -> String {
    let Civil {
        year,
        month,
        day,
        hour,
        minute,
        second,
    } = civil(secs);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}")
}
