use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
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
    tokio::spawn(async move {
        let report = crate::verify::run(
            &folder,
            &config.mutis.verify_command,
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
    let stream = async_stream::stream! { let mut rx=ReceiverStream::new(rx); while let Some(line)=rx.next().await { if let Some(json)=line.strip_prefix("__VERIFY__") { yield Ok::<_,Infallible>(axum::response::sse::Event::default().event("verify").data(json)); } else { yield Ok::<_,Infallible>(axum::response::sse::Event::default().event("log").json_data(json!({"text":line})).unwrap()); } } };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
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
