use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::Response,
};
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use serde::Deserialize;
use std::io::{Read, Write};
use tokio::sync::mpsc;

#[derive(Deserialize)]
pub(crate) struct Params {
    folder: Option<String>,
}

#[derive(Deserialize)]
struct Resize {
    cols: u16,
    rows: u16,
}

fn size(cols: u16, rows: u16) -> PtySize {
    PtySize { rows: rows.max(1), cols: cols.max(1), pixel_width: 0, pixel_height: 0 }
}

fn shell() -> CommandBuilder {
    #[cfg(windows)]
    let mut cmd = {
        let mut cmd = CommandBuilder::new("powershell.exe");
        cmd.arg("-NoLogo");
        cmd
    };
    #[cfg(not(windows))]
    let mut cmd = CommandBuilder::new(std::env::var("SHELL").unwrap_or_else(|_| "sh".into()));
    cmd.env("TERM", "xterm-256color");
    cmd
}

/// A WebSocket is not covered by the CORS-style origin check on POST, so a page from another site could open a shell:
/// only this server's own origin (and the web dev server in a source checkout) may connect.
fn origin_allowed(headers: &HeaderMap, state: &AppState) -> bool {
    let origin = headers.get("origin").and_then(|v| v.to_str().ok()).and_then(|o| o.strip_prefix("http://"));
    let host = headers.get("host").and_then(|v| v.to_str().ok());
    let dev_web = state.dev_repo.is_some() && origin.is_some_and(|o| o == "localhost:7880" || o == "127.0.0.1:7880");
    dev_web || (origin.is_some() && origin == host)
}

/// A real shell on a pseudo-terminal, bridged to the page: binary frames carry keystrokes in and output out,
/// a text frame is `{"cols":n,"rows":n}` to resize. The shell dies with the socket.
pub(crate) async fn terminal(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<Params>,
) -> ApiResult<Response> {
    if !origin_allowed(&headers, &state) {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Forbidden"));
    }
    Ok(ws.on_upgrade(move |socket| session(socket, params.folder)))
}

async fn session(mut socket: WebSocket, folder: Option<String>) {
    let pair = match native_pty_system().openpty(size(80, 24)) {
        Ok(pair) => pair,
        Err(error) => return fail(socket, &error.to_string()).await,
    };
    let mut cmd = shell();
    if let Some(dir) = folder.filter(|dir| std::path::Path::new(dir).is_dir()) {
        cmd.cwd(dir);
    }
    let mut child = match pair.slave.spawn_command(cmd) {
        Ok(child) => child,
        Err(error) => return fail(socket, &error.to_string()).await,
    };
    drop(pair.slave);
    let (Ok(mut reader), Ok(mut writer)) = (pair.master.try_clone_reader(), pair.master.take_writer()) else {
        let _ = child.kill();
        return;
    };
    let (tx, mut rx) = mpsc::channel::<Vec<u8>>(64);
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.blocking_send(buf[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(bytes) => if socket.send(Message::Binary(bytes.into())).await.is_err() { break },
                None => break,
            },
            msg = socket.recv() => match msg {
                // shortcut: a blocking write on the async task; keystrokes are tiny, a pasted megabyte would stall this socket only.
                Some(Ok(Message::Binary(bytes))) => if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() { break },
                Some(Ok(Message::Text(text))) => if let Ok(r) = serde_json::from_str::<Resize>(text.as_str()) {
                    let _ = pair.master.resize(size(r.cols, r.rows));
                },
                Some(Ok(Message::Ping(_) | Message::Pong(_))) => {}
                _ => break,
            },
        }
    }
    let _ = child.kill();
}

async fn fail(mut socket: WebSocket, message: &str) {
    let _ = socket.send(Message::Binary(format!("\r\n{message}\r\n").into_bytes().into())).await;
}
