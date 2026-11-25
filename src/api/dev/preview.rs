//! The live preview server: the Next.js dev server of `web/` (port 7880). The preview panel starts and stops it, so no
//! forgotten copy keeps the port. Only a server this module started is ever stopped; a foreign process on the port is
//! reported, never killed.
//! Máy chủ xem trước được gắn với vòng đời của ứng dụng.
use super::dev_repo_path;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr},
    process::Stdio,
    sync::OnceLock,
    time::Duration,
};
use tokio::{net::TcpStream, process::Child, sync::Mutex};

const PORT: u16 = 7880;
static CHILD: OnceLock<Mutex<Option<Child>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Child>> {
    CHILD.get_or_init(Default::default)
}

/// Dev servers of other projects this app started, by port. Same rule as `CHILD`: only these are ever stopped.
static PROJECTS: OnceLock<Mutex<HashMap<u16, Child>>> = OnceLock::new();

fn projects() -> &'static Mutex<HashMap<u16, Child>> {
    PROJECTS.get_or_init(Default::default)
}

/// True when `package.json` text declares a string `scripts.dev`; that script is the only command ever run for a project.
fn has_dev_script(package_json: &str) -> bool {
    serde_json::from_str::<Value>(package_json)
        .ok()
        .is_some_and(|json| json.pointer("/scripts/dev").is_some_and(Value::is_string))
}

/// Arguments after `npm`: the project's own `dev` script with the port appended (Next and Vite both read `--port`;
/// `PORT` is also set for tools that read the environment).
fn dev_args(port: u16) -> Vec<String> {
    ["run", "dev", "--", "--port"].iter().map(|part| part.to_string()).chain([port.to_string()]).collect()
}

fn local_addresses(port: u16, host: &LocalHost) -> Vec<SocketAddr> {
    let local = [SocketAddr::from((Ipv4Addr::LOCALHOST, port)), SocketAddr::from((Ipv6Addr::LOCALHOST, port))];
    match host {
        LocalHost::Localhost => local.to_vec(),
        LocalHost::Ipv4 => local[..1].to_vec(),
        LocalHost::Ipv6 => local[1..].to_vec(),
    }
}

/// Drops a project child that has already exited.
fn prune(map: &mut HashMap<u16, Child>, port: u16) {
    if map.get_mut(&port).is_some_and(|child| child.try_wait().ok().flatten().is_some()) {
        map.remove(&port);
    }
}

async fn listening() -> bool {
    listening_on(&[SocketAddr::from((Ipv4Addr::LOCALHOST, PORT))]).await
}

async fn listening_on(addresses: &[SocketAddr]) -> bool {
    matches!(
        tokio::time::timeout(Duration::from_millis(500), TcpStream::connect(addresses)).await,
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

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum LocalHost {
    Localhost,
    Ipv4,
    Ipv6,
}

#[derive(Deserialize)]
pub(crate) struct PreviewStatusQuery {
    port: Option<u16>,
    host: Option<LocalHost>,
}

pub(crate) async fn dev_preview_status(
    State(state): State<AppState>,
    Query(query): Query<PreviewStatusQuery>,
) -> ApiResult<Json<Value>> {
    if let Some(port) = query.port {
        let addresses = local_addresses(port, &query.host.unwrap_or(LocalHost::Localhost));
        let mut map = projects().lock().await;
        prune(&mut map, port);
        return Ok(Json(json!({ "port": port, "running": listening_on(&addresses).await, "owned": map.contains_key(&port) })));
    }
    dev_repo_path(&state)?;
    let mut guard = slot().lock().await;
    let owned = owned(&mut guard).await;
    Ok(Json(status(owned).await))
}

#[derive(Deserialize)]
pub(crate) struct Action {
    action: String,
    /// Another project's dev server: its port and the chat folder to run `npm run dev` in. Absent: MutiS's own preview.
    port: Option<u16>,
    folder: Option<String>,
}

/// Starts or stops the `npm run dev` server of a project folder on `port`. Never runs anything but that script.
async fn project_action(action: &str, port: u16, folder: Option<String>) -> ApiResult<Json<Value>> {
    if port < 1024 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Cổng không hợp lệ"));
    }
    let addresses = local_addresses(port, &LocalHost::Localhost);
    let mut map = projects().lock().await;
    prune(&mut map, port);
    let state = |owned: bool, running: bool| Json(json!({ "port": port, "running": running, "owned": owned }));
    match action {
        "start" => {
            if map.contains_key(&port) {
                return Ok(state(true, listening_on(&addresses).await));
            }
            if listening_on(&addresses).await {
                return Err(ApiError::new(StatusCode::CONFLICT, format!("Cổng {port} đang có tiến trình khác chạy; hãy tắt nó trước")));
            }
            let dir = std::path::PathBuf::from(folder.unwrap_or_default());
            let manifest = tokio::fs::read_to_string(dir.join("package.json")).await.unwrap_or_default();
            if !dir.is_dir() || !has_dev_script(&manifest) {
                return Err(ApiError::new(StatusCode::NOT_FOUND, "Thư mục dự án không có script dev trong package.json"));
            }
            let mut command = tokio::process::Command::new(if cfg!(windows) { "npm.cmd" } else { "npm" });
            command.args(dev_args(port)).env("PORT", port.to_string()).current_dir(&dir)
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).kill_on_drop(true);
            crate::core::process::configure_hidden(&mut command);
            let child = command.spawn().map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, format!("Không bật được máy chủ xem trước: {error}")))?;
            crate::core::process::tie_to_app(&child);
            map.insert(port, child);
            Ok(state(true, false))
        }
        "stop" => {
            let Some(mut child) = map.remove(&port) else {
                let message = if listening_on(&addresses).await { format!("Cổng {port} do tiến trình không phải của MutiS giữ; hãy tự tắt nó") } else { "Máy chủ xem trước đang tắt".into() };
                return Err(ApiError::new(StatusCode::CONFLICT, message));
            };
            crate::core::process::kill_process_tree(child.id()).await;
            let _ = tokio::time::timeout(Duration::from_secs(5), child.wait()).await;
            Ok(state(false, listening_on(&addresses).await))
        }
        _ => Err(ApiError::new(StatusCode::BAD_REQUEST, "action phải là start hoặc stop")),
    }
}

pub(crate) async fn dev_preview_action(State(state): State<AppState>, Json(input): Json<Action>) -> ApiResult<Json<Value>> {
    // Another project's server is not tied to dev mode: whoever opened the preview may start or stop what it started.
    if let Some(port) = input.port.filter(|port| *port != PORT) {
        return project_action(&input.action, port, input.folder).await;
    }
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
            crate::core::process::tie_to_app(&child);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_script_is_required_and_must_be_a_string() {
        assert!(has_dev_script(r#"{"scripts":{"dev":"next dev"}}"#));
        assert!(!has_dev_script(r#"{"scripts":{"build":"x"}}"#));
        assert!(!has_dev_script(r#"{"scripts":{"dev":1}}"#));
        assert!(!has_dev_script("not json"));
    }

    #[test]
    fn dev_args_pass_the_port_to_the_script() {
        assert_eq!(dev_args(3000), ["run", "dev", "--", "--port", "3000"]);
    }

    #[test]
    fn host_picks_loopback_families() {
        assert_eq!(local_addresses(3000, &LocalHost::Localhost).len(), 2);
        assert!(local_addresses(3000, &LocalHost::Ipv6)[0].is_ipv6());
    }

    #[tokio::test]
    async fn project_actions_reject_bad_input() {
        assert!(project_action("start", 80, None).await.is_err());
        assert!(project_action("start", 59999, Some("Z:/nope".into())).await.is_err());
        assert!(project_action("stop", 59998, None).await.is_err());
        assert!(project_action("x", 59997, None).await.is_err());
    }
}
