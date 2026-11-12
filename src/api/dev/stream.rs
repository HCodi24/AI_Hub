//! Server-sent-event helpers shared by the dev build endpoints.

use axum::response::{sse::KeepAlive, sse::Sse, IntoResponse, Response};
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::convert::Infallible;
use tokio::{io::AsyncBufReadExt, sync::broadcast, sync::mpsc};
use tokio_stream::wrappers::ReceiverStream;

use crate::app::AppState;
use axum::extract::State;

pub(super) async fn stream_process(
    mut command: tokio::process::Command,
    tx: mpsc::Sender<String>,
) -> bool {
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut tasks = Vec::new();
    if let Some(stream) = stdout {
        tasks.push(tokio::spawn(forward_process_lines(stream, tx.clone())));
    }
    if let Some(stream) = stderr {
        tasks.push(tokio::spawn(forward_process_lines(stream, tx.clone())));
    }
    for task in tasks {
        let _ = task.await;
    }
    child.wait().await.is_ok_and(|status| status.success())
}

async fn forward_process_lines<R>(stream: R, tx: mpsc::Sender<String>)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    let mut lines = tokio::io::BufReader::new(stream).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let _ = tx
            .send(json!({"event":"log","data":{"text":line}}).to_string())
            .await;
    }
}

pub(super) fn sse_channel(rx: mpsc::Receiver<String>) -> Response {
    let stream = async_stream::stream! {
        let mut rx = ReceiverStream::new(rx);
        while let Some(line) = rx.next().await {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event(value["event"].as_str().unwrap_or("log"))
                        .data(value["data"].to_string()),
                );
            } else {
                yield Ok::<_, Infallible>(
                    axum::response::sse::Event::default()
                        .event("log")
                        .json_data(json!({"text": line}))
                        .unwrap(),
                );
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}

pub(crate) async fn dev_events(State(state): State<AppState>) -> Response {
    let mut rx = state.dev_events.subscribe();
    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(message) => {
                    let parsed: Value = serde_json::from_str(&message).unwrap_or(Value::Null);
                    let name = parsed.get("event").and_then(Value::as_str).unwrap_or("message");
                    yield Ok::<_, Infallible>(
                        axum::response::sse::Event::default()
                            .event(name)
                            .data(parsed.get("data").cloned().unwrap_or(Value::Null).to_string()),
                    );
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    };
    Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response()
}
