use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    convert::Infallible,
    path::PathBuf,
    process::Stdio,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use tokio::{io::AsyncBufReadExt, sync::broadcast, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;

pub(crate) async fn dev_status(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"dev":state.dev_repo.is_some()}))
}

pub(crate) async fn dev_repo_info(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    let output = git_output(&repo, &["status", "--porcelain"]).await?;
    let changed = String::from_utf8_lossy(&output.stdout).lines().count();
    let branch = git_text(&repo, &["branch", "--show-current"]).await?;
    let commit = git_text(&repo, &["log", "-1", "--format=%h%x1f%s%x1f%an"]).await?;
    let mut fields = commit.split('\u{1f}');
    Ok(Json(json!({
        "path": repo.to_string_lossy(),
        "branch": branch,
        "changed_files": changed,
        "last_commit": {
            "hash": fields.next().unwrap_or(""),
            "subject": fields.next().unwrap_or(""),
            "author": fields.next().unwrap_or(""),
        }
    })))
}

pub(crate) fn dev_repo_path(state: &AppState) -> ApiResult<PathBuf> {
    state
        .dev_repo
        .clone()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chế độ phát triển đang tắt"))
}

pub(crate) fn effective_chat_folder(
    dev_mode: bool,
    dev_chat: bool,
    client_folder: &str,
    repo: &str,
) -> String {
    if dev_mode && dev_chat {
        repo.to_owned()
    } else {
        client_folder.to_owned()
    }
}

pub(crate) fn dev_chat_preamble(repo: &str) -> String {
    format!(
        concat!(
            "Bạn đang sửa chính ứng dụng AI Hub (repo: {repo}) đang chạy ở chế độ dev.\n",
            "- Giao diện React ở web/src cần người dùng bấm 'Build giao diện' (trang tự tải lại ",
            "khi build xong); code Rust ở src/ cần 'Build & khởi động lại'.\n",
            "- Không chạy `cargo build --release`, không tắt tiến trình ai-hub, ",
            "không sửa target/.\n",
            "- Ghi file bằng UTF-8; giữ nguyên tính năng có sẵn; chạy `cargo test` ",
            "với CARGO_TARGET_DIR=target\\dev-check nếu cần.\n",
            "- Cuối cùng báo: đã sửa file nào, có cần Build & khởi động lại không."
        ),
        repo = repo
    )
}

async fn git_output(repo: &std::path::Path, args: &[&str]) -> ApiResult<std::process::Output> {
    crate::core::process::git_output(repo, args)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không chạy được Git"))
}

async fn git_text(repo: &std::path::Path, args: &[&str]) -> ApiResult<String> {
    let output = crate::core::process::git_output(repo, args)
        .await
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không chạy được Git"))?;
    if !output.status.success() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    // Only trim the end: `git status --porcelain` lines start with a significant space (" M path").
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned())
}

pub(crate) async fn dev_git_diff(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    let stat = git_text(&repo, &["diff", "HEAD", "--stat"]).await?;
    let diff_output = git_output(&repo, &["diff", "--no-color", "HEAD"]).await?;
    let truncated = diff_output.stdout.len() > 200 * 1024;
    let diff =
        String::from_utf8_lossy(&diff_output.stdout[..diff_output.stdout.len().min(200 * 1024)])
            .into_owned();
    let status = git_text(&repo, &["status", "--porcelain", "--untracked-files=all"]).await?;
    let untracked = status
        .lines()
        .filter(|line| line.starts_with("??"))
        .map(|line| line[3..].to_owned())
        .collect::<Vec<_>>();
    let files = status
        .lines()
        .map(|line| line.get(3..).unwrap_or(line).to_owned())
        .collect::<Vec<_>>();
    Ok(Json(
        json!({"stat":stat,"diff":diff,"untracked":untracked,"files":files,"truncated":truncated}),
    ))
}

pub(crate) async fn dev_git_checkpoint(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    ensure_no_jobs(&state)?;
    let status = git_text(&repo, &["status", "--porcelain"]).await?;
    if status.is_empty() {
        return Ok(Json(
            json!({"ok":true,"skipped":true,"message":"Không có thay đổi"}),
        ));
    }
    let ts = crate::dev::iso_now();
    for args in crate::dev::git_checkpoint_args(&ts) {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .await
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không chạy được Git"))?;
        if !output.status.success() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
    }
    Ok(Json(
        json!({"ok":true,"hash":git_text(&repo,&["rev-parse","--short","HEAD"]).await?}),
    ))
}

pub(crate) fn ensure_no_jobs(state: &AppState) -> ApiResult<()> {
    let jobs = state.active_jobs.load(Ordering::SeqCst);
    if jobs > 0 {
        Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("Đang có {jobs} việc AI chạy — chờ xong hoặc dừng"),
        ))
    } else {
        Ok(())
    }
}

pub(crate) async fn dev_git_reset(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    ensure_no_jobs(&state)?;
    let files = git_text(&repo, &["status", "--porcelain", "--untracked-files=all"])
        .await?
        .lines()
        // Report only what reset/clean really discards: tracked changes anywhere, untracked files only under src/ and web/src/.
        .filter(|line| {
            let path = line.get(3..).unwrap_or_default();
            !line.starts_with("??") || path.starts_with("src/") || path.starts_with("web/src/")
        })
        .map(|line| line.get(3..).unwrap_or(line).to_owned())
        .collect::<Vec<_>>();
    for args in crate::dev::git_reset_args() {
        let argv = args.iter().map(String::as_str).collect::<Vec<_>>();
        let output = git_output(&repo, &argv).await?;
        if !output.status.success() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
    }
    Ok(Json(json!({"ok":true,"files":files})))
}

#[derive(Deserialize)]
pub(crate) struct DevCommitInput {
    message: String,
}
pub(crate) async fn dev_git_commit(
    State(state): State<AppState>,
    Json(input): Json<DevCommitInput>,
) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    ensure_no_jobs(&state)?;
    let message = input.message.trim();
    if message.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Nhập nội dung commit",
        ));
    }
    for args in crate::dev::git_commit_args(message) {
        let output = tokio::process::Command::new("git")
            .args(args)
            .current_dir(&repo)
            .output()
            .await
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không chạy được Git"))?;
        if !output.status.success() {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                String::from_utf8_lossy(&output.stderr).to_string(),
            ));
        }
    }
    Ok(Json(
        json!({"ok":true,"hash":git_text(&repo,&["rev-parse","--short","HEAD"]).await?}),
    ))
}

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

/// Build the React UI (`npm run build` in web/). The dev watcher then sees web/out change and reloads the page.
pub(crate) async fn dev_web_build(State(state): State<AppState>) -> ApiResult<Response> {
    let repo = dev_repo_path(&state)?;
    let (tx, rx) = mpsc::channel::<String>(64);
    tokio::spawn(async move {
        let mut cmd = if cfg!(windows) {
            let mut cmd = tokio::process::Command::new("cmd");
            cmd.args(["/C", "npm", "run", "build"]);
            cmd
        } else {
            let mut cmd = tokio::process::Command::new("npm");
            cmd.args(["run", "build"]);
            cmd
        };
        cmd.current_dir(repo.join("web"))
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let ok = stream_process(cmd, tx.clone()).await;
        let _ = tx
            .send(json!({"event":"summary","data":{"web_ok":ok}}).to_string())
            .await;
    });
    Ok(sse_channel(rx))
}

async fn stream_process(mut command: tokio::process::Command, tx: mpsc::Sender<String>) -> bool {
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut tasks = Vec::new();
    if let Some(stream) = stdout {
        tasks.push(tokio::spawn(forward_process_lines(stream, tx.clone())));
    }
    if let Some(stream) = stderr {
        tasks.push(tokio::spawn(forward_process_lines(stream, tx.clone())));
    }
    for task in tasks {
        let _ = task.await;
    }
    child.wait().await.is_ok_and(|status| status.success())
}

async fn forward_process_lines<R>(stream: R, tx: mpsc::Sender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = tx
            .send(json!({"event":"log","data":{"text":line}}).to_string())
            .await;
    }
}

fn sse_channel(rx: mpsc::Receiver<String>) -> Response {
    let stream = async_stream::stream! {
        let mut rx = ReceiverStream::new(rx);
        while let Some(line) = rx.next().await {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event(value["event"].as_str().unwrap_or("log"))
                        .data(value["data"].to_string()),
                );
            } else {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event("log")
                        .json_data(json!({"text": line}))
                        .unwrap(),
                );
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
pub(crate) async fn dev_settings(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"auto_rebuild":state.config.read().await.self_dev.auto_rebuild}))
}
pub(crate) async fn put_dev_settings(
    State(state): State<AppState>,
    Json(value): Json<config::SelfDevConfig>,
) -> ApiResult<Json<config::SelfDevConfig>> {
    if state.dev_repo.is_none() {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Chế độ phát triển đang tắt",
        ));
    }
    let mut cfg = state.config.write().await;
    cfg.self_dev = value.clone();
    config::save(&state.config_path, &cfg)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cài đặt"))?;
    Ok(Json(value))
}

pub(crate) async fn dev_events(State(state): State<AppState>) -> Response {
    let mut rx = state.dev_events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(message) => {
                    let parsed: Value = serde_json::from_str(&message).unwrap_or(Value::Null);
                    let name = parsed.get("event").and_then(Value::as_str).unwrap_or("message");
                    yield Ok::<_, Infallible>(
                        axum::response::sse::Event::default()
                            .event(name)
                            .data(parsed.get("data").cloned().unwrap_or(Value::Null).to_string()),
                    );
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub(crate) fn spawn_dev_watcher(state: AppState, repo: PathBuf) {
    tokio::spawn(async move {
        let mut old_rust = crate::dev::snapshot(&repo);
        let mut old_web = crate::dev::snapshot_web_src(&repo);
        let mut old_out = crate::dev::snapshot_web_out(&repo);
        let mut out_changed_at: Option<Instant> = None;
        let mut pending: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let rust = crate::dev::snapshot(&repo);
            let rust_changes = crate::dev::changed_files(&old_rust, &rust);
            let web = crate::dev::snapshot_web_src(&repo);
            let web_changes = crate::dev::changed_files(&old_web, &web);
            if !web_changes.is_empty() {
                let _ = state.dev_events.send(
                    json!({"event":"web_changed","data":{"files":web_changes.len()}}).to_string(),
                );
            }
            old_web = web;
            let out = crate::dev::snapshot_web_out(&repo);
            if !crate::dev::changed_files(&old_out, &out).is_empty() {
                out_changed_at = Some(Instant::now());
            }
            old_out = out;
            // Reload the page only once the new export is finished: unchanged for 3 seconds and index.html is back.
            if out_changed_at.is_some_and(|at| at.elapsed() >= Duration::from_secs(3))
                && repo.join("web/out/index.html").is_file()
            {
                out_changed_at = None;
                let _ = state
                    .dev_events
                    .send(json!({"event":"ui_changed","data":{}}).to_string());
            }
            if !rust_changes.is_empty() {
                let files = rust_changes
                    .iter()
                    .map(|p| {
                        p.strip_prefix(&repo)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .to_string()
                    })
                    .collect::<Vec<_>>();
                let _ = state
                    .dev_events
                    .send(json!({"event":"rust_changed","data":{"files":files}}).to_string());
                pending = Some(Instant::now() + Duration::from_secs(3));
            }
            old_rust = rust;
            if pending.is_some_and(|at| Instant::now() >= at) {
                pending = None;
                if state.config.read().await.self_dev.auto_rebuild
                    && state.active_jobs.load(Ordering::SeqCst) == 0
                {
                    let _ = run_dev_build(&state, &repo).await;
                }
            }
        }
    });
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

async fn run_dev_build(state: &AppState, repo: &std::path::Path) -> Value {
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
    use super::effective_chat_folder;

    #[test]
    fn dev_chat_folder_uses_repo_only_when_dev_mode_is_on() {
        assert_eq!(
            effective_chat_folder(true, true, "C:/client", "C:/repo"),
            "C:/repo"
        );
        assert_eq!(
            effective_chat_folder(true, false, "C:/client", "C:/repo"),
            "C:/client"
        );
        assert_eq!(
            effective_chat_folder(false, true, "C:/saved", "C:/repo"),
            "C:/saved"
        );
        assert_eq!(
            effective_chat_folder(false, false, "C:/client", "C:/repo"),
            "C:/client"
        );
    }
}
