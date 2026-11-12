use crate::api::{ApiError, ApiResult};
use axum::{http::StatusCode, Json};
use serde_json::{json, Value};

/// One level of a folder, folders first, so the UI can expand the tree lazily.
/// shortcut: any readable path is listed (local single-user app), restrict to the chat folder if the server is ever exposed.
pub(crate) async fn list_dir(
    axum::extract::Query(q): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    let path = q.get("path").cloned().unwrap_or_default();
    let mut entries: Vec<(bool, String)> = std::fs::read_dir(&path)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Không đọc được thư mục"))?
        .flatten()
        .map(|e| (e.path().is_dir(), e.file_name().to_string_lossy().into_owned()))
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.to_lowercase().cmp(&b.1.to_lowercase())));
    Ok(Json(json!({ "entries": entries.into_iter().map(|(dir, name)| json!({ "name": name, "dir": dir })).collect::<Vec<_>>() })))
}

/// Native "choose folder" dialog on the machine running the app. `{"path": null}` when the user cancels.
/// shortcut: Windows only (PowerShell + WinForms), add a platform dialog when the app ships elsewhere.
pub(crate) async fn pick_folder() -> ApiResult<Json<Value>> {
    #[cfg(windows)]
    {
        use crate::core::process::{configure_hidden, run_with_timeout};
        use std::time::Duration;
        // A topmost owner form keeps the dialog in front of the browser window.
        const SCRIPT: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
            Add-Type -AssemblyName System.Windows.Forms;\
            $owner=New-Object System.Windows.Forms.Form -Property @{TopMost=$true};\
            $dialog=New-Object System.Windows.Forms.FolderBrowserDialog;\
            if ($dialog.ShowDialog($owner) -eq 'OK') { $dialog.SelectedPath }";
        let mut command = tokio::process::Command::new("powershell");
        command.args(["-NoProfile", "-STA", "-Command", SCRIPT]);
        configure_hidden(&mut command);
        let output = run_with_timeout(&mut command, Duration::from_secs(600))
            .await
            .map_err(|_| {
                ApiError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Không mở được hộp thoại chọn thư mục",
                )
            })?;
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok(Json(json!({ "path": (!path.is_empty()).then_some(path) })))
    }
    #[cfg(not(windows))]
    Err(ApiError::new(
        StatusCode::NOT_IMPLEMENTED,
        "Chỉ hỗ trợ trên Windows",
    ))
}
