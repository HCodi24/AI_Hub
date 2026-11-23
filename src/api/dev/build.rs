//! Dev build endpoints: cargo test, release build, web build and rebuild.

use super::dev_repo_path;
use super::stream::{sse_channel, stream_process};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::State,
    Json,
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

/// Raises the last number of the first `<key> "x.y.z"` in `text` (Cargo.toml `version = `, tauri.conf.json `"version": `); returns the new version.
fn bump_patch(text: &str, key: &str) -> Option<(String, String)> {
    let start = text.find(key)? + key.len();
    let quote = start + text[start..].find('"')? + 1;
    let end = quote + text[quote..].find('"')?;
    let mut parts: Vec<u64> = text[quote..end].split('.').map(|n| n.parse().ok()).collect::<Option<_>>()?;
    *parts.last_mut()? += 1;
    let version = parts.iter().map(u64::to_string).collect::<Vec<_>>().join(".");
    Some((format!("{}{}{}", &text[..quote], version, &text[end..]), version))
}

/// True while the desktop build runs: the build edits Cargo.toml and web/out, which would make the dev watcher rebuild and restart the server mid-build.
pub(super) static DESKTOP_BUILDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

struct DesktopBuildGuard;

impl Drop for DesktopBuildGuard {
    fn drop(&mut self) {
        DESKTOP_BUILDING.store(false, Ordering::SeqCst);
    }
}

/// Dev-only desktop build, artifact checks and publishing.
pub(crate) async fn dev_desktop_build(State(state): State<AppState>, body: Option<Json<Value>>) -> ApiResult<Response> {
    // The signing-key password lives only in this request and the child process environment.
    let body = body.map(|Json(value)| value).unwrap_or_default();
    let action = match body["action"].as_str().unwrap_or("build") {
        action @ ("build" | "check" | "publish") => action.to_owned(),
        _ => return Err(ApiError::new(StatusCode::BAD_REQUEST, "Thao tác không hợp lệ")),
    };
    let password = body["password"].as_str().filter(|p| !p.is_empty()).map(str::to_owned);
    let repo = dev_repo_path(&state)?;
    if DESKTOP_BUILDING.swap(true, Ordering::SeqCst) {
        return Err(ApiError::new(StatusCode::CONFLICT, "Đang build bản cài"));
    }
    let guard = DesktopBuildGuard;
    let files = [("Cargo.toml", "
version ="), ("desktop/Cargo.toml", "
version ="), ("desktop/tauri.conf.json", "\"version\":")];
    let mut version = String::new();
    if action == "build" {
        for (file, key) in files {
            let path = repo.join(file);
            let bumped = std::fs::read_to_string(&path).map_err(|e| format!("Không đọc được {file}: {e}"))
                .and_then(|text| bump_patch(&text, key).ok_or_else(|| format!("Không thấy số phiên bản trong {file}")))
                .and_then(|(next, new_version)| std::fs::write(&path, next).map(|_| new_version).map_err(|e| format!("Không ghi được {file}: {e}")));
            match bumped {
                Ok(new_version) => version = new_version,
                Err(message) => return Err(ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, message)),
            }
        }
    } else {
        let manifest = std::fs::read_to_string(repo.join("desktop/tauri.conf.json"))
            .ok().and_then(|text| serde_json::from_str::<Value>(&text).ok());
        version = manifest.as_ref().and_then(|value| value["version"].as_str())
            .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được phiên bản bộ cài"))?
            .to_owned();
    }
    let (tx, rx) = mpsc::channel::<String>(64);
    tokio::spawn(async move {
        let mut cmd = tokio::process::Command::new("powershell");
        cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "scripts/build-desktop.ps1"])
            .args(["-Action", action.as_str()])
            .current_dir(&repo)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(password) = password.filter(|_| action == "build") {
            cmd.env("TAURI_SIGNING_PRIVATE_KEY_PASSWORD", password);
        }
        let ok = stream_process(cmd, tx.clone()).await;
        drop(guard);
        let _ = tx.send(json!({"event":"summary","data":{"ok":ok,"version":version,"dir":repo.join("dist").to_string_lossy()}}).to_string()).await;
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
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
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

#[cfg(test)]
mod tests {
    use super::bump_patch;

    #[test]
    fn bump_raises_only_the_first_version_number() {
        let toml = "[package]\nname = \"x\"\nversion = \"0.1.9\"\n[dependencies]\ntauri = { version = \"2\" }\n";
        let (text, version) = bump_patch(toml, "\nversion =").unwrap();
        assert_eq!(version, "0.1.10");
        assert!(text.contains("version = \"0.1.10\"") && text.contains("{ version = \"2\" }"));
        let (json, version) = bump_patch("{\n  \"version\": \"0.1.2\",\n}", "\"version\":").unwrap();
        assert_eq!((json.as_str(), version.as_str()), ("{\n  \"version\": \"0.1.3\",\n}", "0.1.3"));
        assert!(bump_patch("version = \"abc\"", "version =").is_none());
    }
}
