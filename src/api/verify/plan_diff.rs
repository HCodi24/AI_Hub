use super::folder::project_folder;
use crate::api::{ApiError, ApiResult};
use axum::{http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(crate) struct PlanDiffRequest {
    folder: String,
}
pub(crate) async fn plan_diff_handler(
    Json(input): Json<PlanDiffRequest>,
) -> ApiResult<Json<Value>> {
    let folder = project_folder(&input.folder)?;
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
