use super::{
    debater::{pick_debater, Debater},
    run::{self, DebateRun},
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat,
    features::debate::Debate,
};
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::Deserialize;
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct DebateInput {
    request: String,
    folder: String,
    #[serde(default)]
    chat_id: Option<String>,
}

pub(crate) async fn start_debate(
    State(state): State<AppState>,
    Json(input): Json<DebateInput>,
) -> ApiResult<Response> {
    if input
        .chat_id
        .as_deref()
        .is_some_and(|id| !chat::valid_id(id))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    let request = input.request.trim().to_owned();
    if request.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy nhập vấn đề cần tranh luận",
        ));
    }
    let folder = std::path::PathBuf::from(input.folder.trim());
    if !folder.is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = folder.to_string_lossy().into_owned();
    let cfg = state.config.read().await.clone();
    let memory = crate::features::memory::memory_context(&state.config_path, &folder, &cfg.memory);
    let codex = pick_debater(&state, &cfg, "codex_cli").await;
    let claude = pick_debater(&state, &cfg, "claude_cli").await;
    let (Some(codex), Some(claude)) = (codex, claude) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tranh luận cần cả Codex CLI và Claude Code CLI đang kết nối (xem Cài đặt › Nguồn AI)",
        ));
    };
    // The judge is a SECOND, fresh Claude session (no shared context with the Claude debater).
    let judge = Debater {
        model: if cfg.mutis.debate_judge_model.trim().is_empty() {
            claude.model.clone()
        } else {
            cfg.mutis.debate_judge_model.trim().to_owned()
        },
        session: None,
        ..claude.clone()
    };
    let max_rounds = cfg.mutis.debate_rounds.clamp(1, 3);
    let record = Debate {
        chat_id: input.chat_id.clone(),
        id: uuid::Uuid::new_v4().to_string(),
        title: request
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(80)
            .collect(),
        request,
        folder,
        created_at: crate::core::time::now_secs(),
        status: "running".into(),
        codex_provider: codex.provider.name.clone(),
        codex_model: codex.model.clone(),
        claude_provider: claude.provider.name.clone(),
        claude_model: claude.model.clone(),
        judge_model: judge.model.clone(),
        max_rounds,
        ..Default::default()
    };
    if let Some(id) = &input.chat_id {
        record.save_chat(&state.chats, id, true).map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không lưu được chat tranh luận",
            )
        })?;
    }
    let _ = state.debates.save(&record);

    let (out, receiver) = mpsc::channel::<SseEvent>(128);
    let cancel = CancellationToken::new();
    tokio::spawn(run::run_debate(DebateRun {
        state,
        chat_id: input.chat_id,
        record,
        debaters: [codex, claude],
        judge,
        mutis: cfg.mutis,
        memory,
        out,
        cancel: cancel.clone(),
    }));

    let stream_cancel = cancel.clone();
    let stream = async_stream::stream! {
        // Closing the page stops the debate (kills both CLIs) like a normal chat.
        let _guard = CancelOnDrop(stream_cancel);
        let mut events = ReceiverStream::new(receiver);
        while let Some(event) = events.next().await {
            yield Ok::<_, Infallible>(event);
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

struct CancelOnDrop(CancellationToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
