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
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let days = seconds.div_euclid(86400);
    let rem = seconds.rem_euclid(86400);
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}
