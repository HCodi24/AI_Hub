use super::folder::project_folder;
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
use serde_json::json;
use std::convert::Infallible;
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
    let folder = project_folder(&input.folder)?;
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
                        .unwrap_or_default(),
                );
            }
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
