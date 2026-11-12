//! Git inspection and mutation endpoints for the dev repo.

use super::{dev_repo_path, ensure_no_jobs};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};

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
