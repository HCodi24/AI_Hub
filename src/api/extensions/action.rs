use super::run::{run_action, RunAction};
use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use std::{convert::Infallible, ffi::OsString, path::PathBuf};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct ExtensionActionInput {
    folder: Option<String>,
}

pub(crate) async fn extension_action(
    State(state): State<AppState>,
    Path((id, action_id)): Path<(String, String)>,
    Json(input): Json<ExtensionActionInput>,
) -> ApiResult<Response> {
    let Some((_, action)) = crate::extensions::action(&id, &action_id) else {
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
            release(&state, &id).await;
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "Cần thư mục hiện tại",
            ));
        };
        let path = PathBuf::from(raw);
        if !path.is_dir() || raw.contains('%') {
            release(&state, &id).await;
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
    let exe = crate::providers::cli_support::resolve_executable(action.command, None);
    let args = action
        .args
        .iter()
        .map(|s| OsString::from(*s))
        .collect::<Vec<_>>();
    let backup = action.backup.map(str::to_owned);
    let configured_backups = crate::core::paths::backups_dir(&state.config_path).ok();
    let modify = action.modifies.to_owned();
    let command = action.command;
    let job_guard = JobGuard::new(state.active_jobs.clone());
    tokio::spawn(async move {
        let _job_guard = job_guard;
        let result = run_action(
            &tx,
            RunAction {
                token,
                exe,
                args,
                folder,
                backup,
                configured_backups,
                modify,
                command,
            },
        )
        .await;
        let _ = tx.send(Err(result.err().unwrap_or_default())).await;
        release(&state2, &id2).await;
    });
    let stream = ReceiverStream::new(rx).flat_map(|item| {
        tokio_stream::iter(events(item).into_iter().map(Ok::<_, Infallible>))
    });
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

async fn release(state: &AppState, id: &str) {
    state.extension_running.lock().await.remove(id);
}

pub(super) fn events(item: Result<String, String>) -> Vec<Event> {
    match item {
        Ok(line) => vec![Event::default()
            .event("log")
            .data(json!(line).to_string())],
        Err(error) => {
            let mut out = Vec::new();
            if !error.is_empty() {
                out.push(
                    Event::default()
                        .event("error")
                        .data(json!({"message":error}).to_string()),
                );
            }
            out.push(
                Event::default()
                    .event("done")
                    .data(json!({"ok":error.is_empty()}).to_string()),
            );
            out
        }
    }
}
