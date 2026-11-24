use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    time::Instant,
};
use tokio::{sync::mpsc::Sender, time::Duration};

mod command;
mod detect;
#[cfg(test)]
mod review;
mod scan;

use command::run_command;
use detect::detect;
use scan::{changed_files, scan_invalid_utf8, scan_mojibake};

pub use detect::{command_for, detected_commands, folder_key};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Serialize)]
pub struct Check {
    pub name: String,
    pub ok: bool,
    pub detail: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct ResultReport {
    pub ok: bool,
    pub summary_lines: Vec<String>,
    pub checks: Vec<Check>,
    pub duration_ms: u64,
    /// True when no build/test command was found or set: only the encoding scans ran, so `ok` proves nothing about the code.
    pub unverified: bool,
}

const UNVERIFIED: &str = "Không có lệnh kiểm tra cho thư mục này: chưa kiểm tra được (cần scripts/verify.ps1, Cargo.toml, package.json hoặc lệnh trong Cài đặt)";

pub async fn run(
    folder: &Path,
    custom: &str,
    ignore: &[String],
    tx: tokio::sync::mpsc::Sender<String>,
) -> ResultReport {
    let started = Instant::now();
    let deadline = started + Duration::from_secs(900);
    let commands = verification_commands(folder, custom);
    let unverified = commands.is_empty();
    let mut checks = verify_commands(folder, commands, deadline, &tx).await;

    let files = changed_files(folder).await;
    checks.push(verify_utf8(folder, &files, &tx).await);
    checks.push(verify_mojibake(folder, &files, ignore, &tx).await);
    build_report(checks, started, unverified)
}

fn verification_commands(folder: &Path, custom: &str) -> Vec<(&'static str, String)> {
    if !custom.trim().is_empty() {
        vec![("verify_command", custom.to_owned())]
    } else {
        detect(folder)
    }
}

async fn verify_commands(
    folder: &Path,
    commands: Vec<(&'static str, String)>,
    deadline: Instant,
    tx: &Sender<String>,
) -> Vec<Check> {
    let mut checks = Vec::new();
    // No command is not a pass: nothing ran, so the report says "not verified" and `unverified` is set for callers.
    if commands.is_empty() {
        let _ = tx.send(UNVERIFIED.into()).await;
        checks.push(Check {
            name: "verify".into(),
            ok: true,
            detail: UNVERIFIED.into(),
        });
    }
    for (name, command) in commands {
        let _ = tx.send(format!("Chạy: {command}")).await;
        let (ok, output) = run_command(folder, &command, deadline, tx).await;
        for line in output
            .lines()
            .rev()
            .take(120)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
        {
            let _ = tx.send(line.to_owned()).await;
        }
        checks.push(Check {
            name: name.into(),
            ok,
            detail: tail(&output, 4000),
        });
    }
    checks
}

async fn verify_utf8(folder: &Path, files: &[PathBuf], tx: &Sender<String>) -> Check {
    let invalid_utf8 = scan_invalid_utf8(folder, files);
    let utf8_ok = invalid_utf8.is_empty();
    let utf8_detail = if utf8_ok {
        "Tất cả tệp văn bản hợp lệ UTF-8".to_owned()
    } else {
        invalid_utf8.join(", ")
    };
    let _ = tx.send(format!("UTF8: {utf8_detail}")).await;
    Check {
        name: "UTF8".into(),
        ok: utf8_ok,
        detail: utf8_detail,
    }
}

async fn verify_mojibake(
    folder: &Path,
    files: &[PathBuf],
    ignore: &[String],
    tx: &Sender<String>,
) -> Check {
    let mojibake = scan_mojibake(folder, files, ignore);
    let ok = mojibake.is_empty();
    let detail = if ok {
        "Không phát hiện mojibake".to_owned()
    } else {
        mojibake
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join("; ")
    };
    let _ = tx.send(format!("MOJIBAKE: {detail}")).await;
    Check {
        name: "MOJIBAKE".into(),
        ok,
        detail,
    }
}

fn build_report(checks: Vec<Check>, started: Instant, unverified: bool) -> ResultReport {
    let ok = checks.iter().all(|c| c.ok);
    let mut summary_lines = checks
        .iter()
        .map(|c| {
            format!(
                "{}: {} — {}",
                c.name,
                if c.detail == UNVERIFIED { "CHƯA KIỂM TRA" } else if c.ok { "ĐẠT" } else { "LỖI" },
                c.detail.lines().last().unwrap_or("")
            )
        })
        .take(12)
        .collect::<Vec<_>>();
    if summary_lines.is_empty() {
        summary_lines.push("Không có kết quả kiểm tra".into());
    }
    ResultReport {
        ok,
        summary_lines,
        checks,
        duration_ms: started.elapsed().as_millis() as u64,
        unverified,
    }
}

fn tail(s: &str, n: usize) -> String {
    s.chars()
        .rev()
        .take(n)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}
