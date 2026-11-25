use super::{forward, picker, validate_port, ApiError, ApiResult, PICKER_PATH};
use axum::{extract::Path, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};
use std::{collections::HashMap, net::Ipv4Addr, sync::LazyLock};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

struct PreviewListener {
    port: u16,
    task: JoinHandle<()>,
}

static LISTENERS: LazyLock<Mutex<HashMap<u16, PreviewListener>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(in crate::api) async fn open(Path(target): Path<String>) -> ApiResult<Json<Value>> {
    let target = validate_port(&target)?;
    let mut listeners = LISTENERS.lock().await;
    if let Some(existing) = listeners.get(&target) {
        if !existing.task.is_finished() {
            return Ok(Json(json!({ "port": existing.port })));
        }
    }

    // Giữ các socket trùng cổng đích đến khi hệ điều hành cấp cổng khác.
    let mut skipped = Vec::new();
    let (socket, port) = loop {
        let socket = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(listener_error)?;
        let port = socket.local_addr().map_err(listener_error)?.port();
        if port != target && !listeners.contains_key(&port) {
            break (socket, port);
        }
        skipped.push(socket);
    };
    let app = Router::new()
        .route(
            PICKER_PATH,
            get(picker).head(|| async { StatusCode::METHOD_NOT_ALLOWED }),
        )
        .fallback(forward)
        .with_state(target);
    let task = tokio::spawn(async move {
        let _ = axum::serve(socket, app).await;
    });
    listeners.insert(target, PreviewListener { port, task });
    Ok(Json(json!({ "port": port })))
}

fn listener_error(_: std::io::Error) -> ApiError {
    ApiError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Không thể mở cổng proxy xem trước",
    )
}
