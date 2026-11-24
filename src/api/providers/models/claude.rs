use super::catalog::ModelCatalog;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    providers,
};
use axum::http::StatusCode;
use serde_json::Value;
use std::{ffi::OsString, path::Path as FsPath, process::Stdio, time::Duration};

/// Model ids from Claude Code: one `list_models` control request over `-p` stream-json (no API turn), like the SDK's `supportedModels()`.
pub(super) async fn claude_models(
    state: &AppState,
    provider: &crate::config::ProviderConfig,
) -> ApiResult<ModelCatalog> {
    let bad_gateway = |error: anyhow::Error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string());
    let executable = providers::cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Claude CLI trên máy"))?;
    let home = providers::claude::environment::home(&state.config_path, provider).map_err(bad_gateway)?;
    let models = list_claude_models(&executable, &home).await.map_err(bad_gateway)?;
    if models.ids.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Claude không trả model nào; hãy đăng nhập Claude rồi thử lại",
        ));
    }
    Ok(models)
}

async fn list_claude_models(executable: &FsPath, home: &FsPath) -> anyhow::Result<ModelCatalog> {
    use anyhow::Context as _;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let args = ["-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"]
        .map(OsString::from);
    let mut command = providers::cli_support::command_for(executable, &args);
    providers::claude::environment::apply(&mut command, home);
    let mut child = command
        .current_dir(home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .context("Không khởi động được Claude CLI")?;
    let mut stdin = child.stdin.take().context("Không mở được kết nối Claude")?;
    let stdout = child.stdout.take().context("Không đọc được kết nối Claude")?;
    let request = serde_json::json!({
        "type": "control_request",
        "request_id": "mutis-list-models",
        "request": {"subtype": "list_models"}
    });
    stdin.write_all(format!("{request}\n").as_bytes()).await?;
    stdin.flush().await?;
    let read = async {
        let mut lines = BufReader::new(stdout).lines();
        while let Some(line) = lines.next_line().await? {
            let Ok(value) = serde_json::from_str::<Value>(&line) else { continue };
            if value["type"] != "control_response" {
                continue;
            }
            let response = &value["response"];
            if response["subtype"] == "error" {
                anyhow::bail!("Claude CLI không hỗ trợ lấy danh sách model; hãy cập nhật Claude CLI");
            }
            if let Some(rows) = response["response"]["models"].as_array() {
                let mut models = ModelCatalog::default();
                for row in rows {
                    models.add_claude(row);
                }
                return Ok(models);
            }
        }
        anyhow::bail!("Claude CLI đã đóng kết nối trước khi trả danh sách model")
    };
    let result = tokio::time::timeout(Duration::from_secs(30), read)
        .await
        .map_err(|_| anyhow::anyhow!("Claude CLI không phản hồi trong 30 giây"))?;
    drop(stdin);
    let _ = child.kill().await;
    result
}
