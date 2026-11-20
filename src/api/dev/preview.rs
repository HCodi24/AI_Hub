//! The live preview server: the Next.js dev server of `web/` (port 7880). The preview panel starts and stops it, so no
//! forgotten copy keeps the port. Only a server this module started is ever stopped; a foreign process on the port is
//! reported, never killed.
use super::dev_repo_path;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{process::Stdio, sync::OnceLock, time::Duration};
use tokio::{net::TcpStream, process::Child, sync::Mutex};

const PORT: u16 = 7880;
static CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Child>> {
    CHILD.get_or_init(Default::default)
}

async fn listening() -> bool {
    matches!(
        tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(("127.0.0.1", PORT))).await,
        Ok(Ok(_))
    )
}

/// `owned`: this app started the running server and can stop it.
async fn status(owned: bool) -> Value {
    json!({ "port": PORT, "running": listening().await, "owned": owned })
}

/// Forgets a child that has already exited, then says whether one is still held.
async fn owned(guard: &mut Option<Child>) -> bool {
    if guard.as_mut().is_some_and(|child| child.try_wait().ok().flatten().is_some()) {
        *guard = None;
    }
    guard.is_some()
}

/// Stops a dev server left behind by an earlier run of the app (it was killed without a chance to stop its child).
/// Only Next dev processes of this repo's own `web/` match, never other processes; the launcher and its server child
/// are stopped together because the launcher respawns the server. Windows only.
async fn stop_leftover(web: &std::path::Path) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let next_dir = web.join("node_modules").join("next").to_string_lossy().replace('\'', "''");
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='node.exe'\" | Where-Object {{ $_.CommandLine -like '*{next_dir}\\*' -and ($_.CommandLine -like '* dev *' -or $_.CommandLine -like '*start-server*') }} | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }}"
    );
    let mut command = tokio::process::Command::new("powershell");
    command.args(["-NoProfile", "-NonInteractive", "-Command", &script]).stdin(Stdio::null());
    crate::core::process::configure_hidden(&mut command);
    let _ = crate::core::process::run_with_timeout(&mut command, Duration::from_secs(15)).await;
    tokio::time::sleep(Duration::from_millis(800)).await;
    !listening().await
}

pub(crate) async fn dev_preview_status(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    dev_repo_path(&state)?;
    let mut guard = slot().lock().await;
    let owned = owned(&mut guard).await;
    Ok(Json(status(owned).await))
}

#[derive(Deserialize)]
pub(crate) struct Action {
    action: String,
}

pub(crate) async fn dev_preview_action(State(state): State<AppState>, Json(input): Json<Action>) -> ApiResult<Json<Value>> {
    let repo = dev_repo_path(&state)?;
    let mut guard = slot().lock().await;
    let held = owned(&mut guard).await;
    match input.action.as_str() {
        "start" => {
            if held {
                return Ok(Json(status(true).await));
            }
            if listening().await {
                return Err(ApiError::new(StatusCode::CONFLICT, format!("Cổng {PORT} đang có tiến trình khác chạy; hãy tắt nó trước")));
            }
            let web = repo.join("web");
            let next = web.join("node_modules/next/dist/bin/next");
            if !next.is_file() {
                return Err(ApiError::new(StatusCode::NOT_FOUND, "Chưa cài thư viện giao diện; chạy npm install trong web"));
            }
            // Node itself (not npm/cmd), so dropping the handle kills the server, not just a launcher.
            let mut command = tokio::process::Command::new("node");
            command.arg(next).args(["dev", "-p", &PORT.to_string()]).current_dir(&web)
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
            crate::core::process::configure_hidden(&mut command);
            let child = command.spawn().map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Không bật được máy chủ xem trước: {error}")))?;
            *guard = Some(child);
            Ok(Json(status(true).await))
        }
        "stop" => {
            let Some(mut child) = guard.take() else {
                if !listening().await {
                    return Err(ApiError::new(StatusCode::CONFLICT, "Máy chủ xem trước đang tắt"));
                }
                if stop_leftover(&repo.join("web")).await {
                    return Ok(Json(status(false).await));
                }
                return Err(ApiError::new(StatusCode::CONFLICT, format!("Cổng {PORT} do tiến trình không thuộc dự án này giữ; hãy tự tắt nó")));
            };
            crate::core::process::kill_process_tree(child.id()).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            Ok(Json(status(false).await))
        }
        _ => Err(ApiError::new(StatusCode::BAD_REQUEST, "action phải là start hoặc stop")),
    }
}
