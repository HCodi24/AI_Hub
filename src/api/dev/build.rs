//! Dev build endpoints: cargo test, release build, web build and rebuild.

use super::dev_repo_path;
use super::stream::{sse_channel, stream_process};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    path::PathBuf,
    process::Stdio,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tokio::{io::AsyncBufReadExt, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;

pub(crate) async fn dev_test(State(state): State<AppState>) -> ApiResult<Response> {
    let repo = dev_repo_path(&state)?;
    let cfg = state.config.read().await.clone();
    let verify = cfg.mutis.verify_gate;
    let command = cfg.mutis.verify_command.clone();
    let ignore = cfg.mutis.verify_ignore.clone();
    let (tx, rx) = mpsc::channel::<String>(64);
    tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args(["test"])
            .current_dir(&repo)
            .env("CARGO_TARGET_DIR", repo.join("target/dev-check"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let result = stream_process(cmd, tx.clone()).await;
        let _ = tx
            .send(json!({"event":"summary","data":{"cargo_ok":result}}).to_string())
            .await;
        if verify {
            let report = crate::verify::run(&repo, &command, &ignore, tx.clone()).await;
            let _ = tx
                .send(json!({"event":"verify","data":report}).to_string())
                .await;
        }
    });
    Ok(sse_channel(rx))
}

pub(crate) async fn dev_release(State(state): State<AppState>) -> ApiResult<Response> {
    let repo = dev_repo_path(&state)?;
    let (tx, rx) = mpsc::channel::<String>(64);
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .map(|path| {
            if path.is_absolute() {
                path
            } else {
                repo.join(path)
            }
        })
        .unwrap_or_else(|| repo.join("target"));
    let out = target.join("release/ai-hub.exe");
    tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new("cargo");
        cmd.args(["build", "--release"])
            .current_dir(&repo)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let ok = stream_process(cmd, tx.clone()).await;
        let _=tx.send(json!({"event":"summary","data":{"ok":ok,"exe":out.to_string_lossy(),"note":"Nếu báo Access is denied, hãy đóng exe release đang chạy trước."}}).to_string()).await;
    });
    Ok(sse_channel(rx))
}

/// `npm run build` in web/ (through `cmd /C` on Windows, where npm is a .cmd file).
pub(super) fn npm_build_command(repo: &std::path::Path) -> tokio::process::Command {
    let mut cmd = if cfg!(windows) {
        let mut cmd = tokio::process::Command::new("cmd");
        cmd.args(["/C", "npm", "run", "build"]);
        cmd
    } else {
        let mut cmd = tokio::process::Command::new("npm");
        cmd.args(["run", "build"]);
        cmd
    };
    cmd.current_dir(repo.join("web"));
    cmd
}

/// Build the React UI (`npm run build` in web/). The dev watcher then sees web/out change and reloads the page.
pub(crate) async fn dev_web_build(State(state): State<AppState>) -> ApiResult<Response> {
    let repo = dev_repo_path(&state)?;
    let (tx, rx) = mpsc::channel::<String>(64);
    tokio::spawn(async move {
        let mut cmd = npm_build_command(&repo);
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let ok = stream_process(cmd, tx.clone()).await;
        let _ = tx
            .send(json!({"event":"summary","data":{"web_ok":ok}}).to_string())
            .await;
    });
    Ok(sse_channel(rx))
}

pub(crate) async fn dev_rebuild(State(state): State<AppState>) -> ApiResult<Response> {
    let repo = state
        .dev_repo
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chế độ phát triển đang tắt"))?;
    let jobs = state.active_jobs.load(Ordering::SeqCst);
    if jobs > 0 {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Đang có {jobs} việc AI chạy — chờ xong hoặc dừng"),
        ));
    }
    let (tx, rx) = mpsc::channel::<String>(64);
    let state2 = state.clone();
    tokio::spawn(async move {
        let result = run_dev_build(&state2, &repo).await;
        let _ = tx.send(result.to_string()).await;
    });
    let stream = async_stream::stream! {
        let mut rx = ReceiverStream::new(rx);
        while let Some(line) = rx.next().await {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let name = value["event"].as_str().unwrap_or("log");
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event(name)
                        .data(value["data"].to_string()),
                );
            } else {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default().event("log").data(line),
                );
            }
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

pub(super) async fn run_dev_build(state: &AppState, repo: &std::path::Path) -> Value {
    let start = Instant::now();
    let spawned = tokio::process::Command::new("cargo")
        .args(["build"])
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let (ok, logs) = match spawned {
        Ok(mut child) => {
            let stdout = child.stdout.take();
            let stderr = child.stderr.take();
            let (tx, mut rx) = mpsc::channel::<String>(128);
            let tx1 = tx.clone();
            let tx2 = tx.clone();
            let out_task = tokio::spawn(async move {
                let mut text = String::new();
                if let Some(stream) = stdout {
                    let mut reader = tokio::io::BufReader::new(stream);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        let clean = crate::core::text::strip_ansi(&line);
                        text.push_str(&clean);
                        let _ = tx1.send(clean).await;
                    }
                }
                text
            });
            let err_task = tokio::spawn(async move {
                let mut text = String::new();
                if let Some(stream) = stderr {
                    let mut reader = tokio::io::BufReader::new(stream);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                            break;
                        }
                        let clean = crate::core::text::strip_ansi(&line);
                        text.push_str(&clean);
                        let _ = tx2.send(clean).await;
                    }
                }
                text
            });
            drop(tx);
            let state_events = state.dev_events.clone();
            let logs_task = tokio::spawn(async move {
                let mut all = String::new();
                while let Some(line) = rx.recv().await {
                    all.push_str(&line);
                    let _ = state_events
                        .send(json!({"event":"build_log","data":{"text":line}}).to_string());
                }
                all
            });
            let status = child.wait().await;
            let out = out_task.await.unwrap_or_default();
            let err = err_task.await.unwrap_or_default();
            let all = logs_task.await.unwrap_or_default();
            (
                status.is_ok_and(|value| value.success()),
                if all.is_empty() {
                    format!("{out}{err}")
                } else {
                    all
                },
            )
        }
        Err(e) => (false, e.to_string()),
    };
    let clean = crate::core::text::strip_ansi(&logs);
    let tail = clean
        .lines()
        .rev()
        .take(40)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n");
    let result = json!({"ok":ok,"duration_ms":start.elapsed().as_millis(),"errors_tail":if ok {""} else {&tail}});
    let event = json!({"event":"rebuild","data":result});
    let _ = state.dev_events.send(event.to_string());
    if ok {
        tokio::time::sleep(Duration::from_millis(300)).await;
        std::process::exit(75);
    }
    result
}
