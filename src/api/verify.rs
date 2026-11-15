use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::{convert::Infallible, path::PathBuf};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

#[derive(Deserialize)]
pub(crate) struct VerifyRequest {
    folder: String,
}
pub(crate) async fn verify_handler(
    State(state): State<AppState>,
    Json(input): Json<VerifyRequest>,
) -> ApiResult<Response> {
    let _job_guard = JobGuard::new(state.active_jobs.clone());
    let config = state.config.read().await.clone();
    if !config.mutis.effective(config.mutis.verify_gate) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "Bật MutiS và Cổng kiểm tra trong Cài đặt",
        ));
    }
    let folder = PathBuf::from(input.folder)
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Thư mục không tồn tại"))?;
    if !folder.is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Đường dẫn không phải thư mục",
        ));
    }
    let (tx, rx) = mpsc::channel::<String>(32);
    let command = crate::verify::command_for(&config.mutis, &folder);
    tokio::spawn(async move {
        let report = crate::verify::run(
            &folder,
            &command,
            &config.mutis.verify_ignore,
            tx.clone(),
        )
        .await;
        let _ = tx
            .send(format!(
                "__VERIFY__{}",
                serde_json::to_string(&report).unwrap_or_default()
            ))
            .await;
    });
    let stream = async_stream::stream! {
        let mut rx = ReceiverStream::new(rx);
        while let Some(line) = rx.next().await {
            if let Some(json) = line.strip_prefix("__VERIFY__") {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default().event("verify").data(json),
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
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

const MAX_PROJECT_COMMANDS: usize = 200;

#[derive(Deserialize)]
pub(crate) struct CommandsQuery {
    folder: String,
}
#[derive(Deserialize)]
pub(crate) struct CommandsInput {
    folder: String,
    command: String,
}

fn project_folder(raw: &str) -> ApiResult<PathBuf> {
    let folder = PathBuf::from(raw)
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Thư mục không tồn tại"))?;
    if !folder.is_dir() {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Đường dẫn không phải thư mục"));
    }
    Ok(folder)
}

/// What the user needs to confirm a project's verify command: the saved one, the global one and what auto-detection would run.
async fn commands_view(state: &AppState, folder: &std::path::Path) -> Value {
    let config = state.config.read().await;
    json!({
        "folder": folder.to_string_lossy(),
        "saved": config.mutis.verify_commands.get(&crate::verify::folder_key(folder)),
        "global": config.mutis.verify_command,
        "detected": crate::verify::detected_commands(folder).into_iter().map(|(name, command)| json!({"name": name, "command": command})).collect::<Vec<_>>(),
    })
}

pub(crate) async fn verify_commands_get(State(state): State<AppState>, Query(query): Query<CommandsQuery>) -> ApiResult<Json<Value>> {
    let folder = project_folder(&query.folder)?;
    Ok(Json(commands_view(&state, &folder).await))
}

/// Saves (or, when empty, removes) the verify command of one project folder.
pub(crate) async fn verify_commands_put(State(state): State<AppState>, Json(input): Json<CommandsInput>) -> ApiResult<Json<Value>> {
    let folder = project_folder(&input.folder)?;
    let command = input.command.trim().to_owned();
    if command.chars().count() > 2000 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Lệnh kiểm tra dài quá 2000 ký tự"));
    }
    {
        let mut config = state.config.write().await;
        let key = crate::verify::folder_key(&folder);
        if command.is_empty() {
            config.mutis.verify_commands.remove(&key);
        } else if config.mutis.verify_commands.contains_key(&key) || config.mutis.verify_commands.len() < MAX_PROJECT_COMMANDS {
            config.mutis.verify_commands.insert(key, command);
        } else {
            return Err(ApiError::new(StatusCode::BAD_REQUEST, "Đã lưu quá nhiều dự án; xóa bớt trước"));
        }
        crate::config::save(&state.config_path, &config)
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cài đặt"))?;
    }
    Ok(Json(commands_view(&state, &folder).await))
}

#[derive(Deserialize)]
pub(crate) struct PlanDiffRequest {
    folder: String,
}
pub(crate) async fn plan_diff_handler(
    Json(input): Json<PlanDiffRequest>,
) -> ApiResult<Json<Value>> {
    let folder = PathBuf::from(input.folder)
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Thư mục không tồn tại"))?;
    let output = tokio::process::Command::new("git")
        .args(["diff", "--no-color", "HEAD"])
        .current_dir(folder)
        .output()
        .await
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Không chạy được git diff"))?;
    let diff = String::from_utf8_lossy(&output.stdout)
        .chars()
        .take(30_000)
        .collect::<String>();
    Ok(Json(json!({"diff":diff})))
}
