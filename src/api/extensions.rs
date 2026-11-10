use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{convert::Infallible, ffi::OsString, path::PathBuf, process::Stdio, time::Duration};
use tokio::{io::AsyncBufReadExt, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct ExtensionQuery {
    folder: Option<String>,
}
#[derive(Deserialize)]
pub(crate) struct ExtensionActionInput {
    folder: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct StopResponse {
    stopped: bool,
}

pub(crate) async fn extension_list(
    axum::extract::Query(q): axum::extract::Query<ExtensionQuery>,
) -> Json<Vec<crate::extensions::Card>> {
    Json(crate::extensions::status(q.folder.as_deref()).await)
}
pub(crate) async fn extension_stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<StopResponse> {
    let token = state.extension_running.lock().await.get(&id).cloned();
    if let Some(token) = token {
        token.cancel();
        Json(StopResponse { stopped: true })
    } else {
        Json(StopResponse { stopped: false })
    }
}
pub(crate) async fn extension_action(
    State(state): State<AppState>,
    Path((id, action_id)): Path<(String, String)>,
    Json(input): Json<ExtensionActionInput>,
) -> ApiResult<Response> {
    let Some((definition, action)) = crate::extensions::action(&id, &action_id) else {
        return Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "Không tìm thấy tiện ích hoặc thao tác",
        ));
    };
    let mut running = state.extension_running.lock().await;
    if running.contains_key(&id) {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "Extension đang có tác vụ",
        ));
    }
    let token = CancellationToken::new();
    running.insert(id.clone(), token.clone());
    drop(running);
    let mut folder = None;
    if action.needs_folder {
        let Some(raw) = input.folder.as_deref() else {
            state.extension_running.lock().await.remove(&id);
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Cần thư mục hiện tại",
            ));
        };
        let path = PathBuf::from(raw);
        if !path.is_dir() || raw.contains('%') {
            state.extension_running.lock().await.remove(&id);
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Thư mục không hợp lệ hoặc không tồn tại",
            ));
        }
        folder = Some(path);
    }
    let (tx, rx) = mpsc::channel::<Result<String, String>>(64);
    let state2 = state.clone();
    let id2 = id.clone();
    let exe = crate::extensions::resolve(action.command).or_else(|| {
        if action.command == "npm" {
            crate::extensions::resolve("npm.cmd")
        } else {
            None
        }
    });
    let args = action
        .args
        .iter()
        .map(|s| OsString::from(*s))
        .collect::<Vec<_>>();
    let backup = action.backup.map(str::to_owned);
    let configured_backups = crate::core::paths::backups_dir(&state.config_path).ok();
    let modify = action.modifies.to_owned();
    let job_guard = JobGuard::new(state.active_jobs.clone());
    tokio::spawn(async move {
        let _job_guard = job_guard;
        let result = async {
            if let Some(file) = backup.as_deref() {
                if let Some(home) = dirs::home_dir() {
                    let source = home.join(file);
                    if source.exists() {
                        let dir = configured_backups.clone().unwrap_or_else(|| {
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
            let path =
                exe.ok_or_else(|| format!("Kh\u{00f4}ng t\u{00ec}m th\u{1ea5}y executable: {}", action.command))?;
            let _ = tx
                .send(Ok(format!(
                    "{} {} | c\u{00f3} th\u{1ec3} s\u{1eeda}: {}",
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
                .ok_or_else(|| "Kh\u{00f4}ng \u{0111}\u{1ecdc} \u{0111}\u{01b0}\u{1ee3}c output".to_string())?;
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
                            return Err("\u{0110}\u{00e3} d\u{1eeb}ng".to_string());
                        }
                        _ = &mut deadline => {
                            providers::cli_support::terminate_child_tree(&mut child).await;
                            return Err("Qu\u{00e1} th\u{1edd}i gian ch\u{1edd} 10 ph\u{00fa}t".to_string());
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
                    Err(format!("L\u{1ec7}nh k\u{1ebf}t th\u{00fa}c v\u{1edb}i {status}"))
                }
            }
            .await;
            wait
        }
        .await;
        let _ = tx.send(Err(result.err().unwrap_or_default())).await;
        state2.extension_running.lock().await.remove(&id2);
    });
    let stream = ReceiverStream::new(rx).flat_map(|item| {
        let events = match item {
            Ok(line) => vec![axum::response::sse::Event::default()
                .event("log")
                .data(json!(line).to_string())],
            Err(error) => {
                let mut out = Vec::new();
                if !error.is_empty() {
                    out.push(
                        axum::response::sse::Event::default()
                            .event("error")
                            .data(json!({"message":error}).to_string()),
                    );
                }
                out.push(
                    axum::response::sse::Event::default()
                        .event("done")
                        .data(json!({"ok":error.is_empty()}).to_string()),
                );
                out
            }
        };
        tokio_stream::iter(events.into_iter().map(Ok::<_, Infallible>))
    });
    let _ = definition;
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
fn backup_timestamp() -> String {
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
#[cfg(test)]
mod extension_tests {
    #[test]
    fn ansi_sequences_are_removed() {
        assert_eq!(
            crate::core::text::strip_ansi("\u{1b}[31mred\u{1b}[0m"),
            "red"
        );
    }
    #[test]
    fn backup_timestamp_has_required_shape() {
        let stamp = super::backup_timestamp();
        assert_eq!(stamp.len(), 15);
        assert_eq!(stamp.as_bytes()[8], b'-');
    }
}
