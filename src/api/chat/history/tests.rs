use super::*;
use crate::app::RunningChat;

fn state() -> (AppState, String) {
    let root = std::env::temp_dir().join(format!("mutis-compact-api-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let state = AppState::new(crate::config::Config::seeded(), root.join("config.json"), root.join("chats"), None, 0);
    let id = uuid::Uuid::new_v4().to_string();
    let record: ChatRecord = serde_json::from_value(serde_json::json!({
        "id": id, "title": "t", "provider_id": "codex-cli", "model": "m", "folder": root.to_string_lossy(), "messages": [], "updated_at": 1
    })).unwrap();
    state.chats.save(&record).unwrap();
    (state, id)
}

#[tokio::test]
async fn compact_refuses_bad_ids_unknown_chats_and_running_chats() {
    let (state, id) = state();
    let status = |result: ApiResult<Json<CompactResponse>>| result.err().map(|e| e.status);
    assert_eq!(status(compact_chat(State(state.clone()), Path("not-a-uuid".into())).await), Some(StatusCode::BAD_REQUEST));
    assert_eq!(status(compact_chat(State(state.clone()), Path(uuid::Uuid::new_v4().to_string())).await), Some(StatusCode::NOT_FOUND));
    state.running.lock().await.insert(id.clone(), RunningChat { request_id: uuid::Uuid::new_v4(), cancel: tokio_util::sync::CancellationToken::new() });
    assert_eq!(status(compact_chat(State(state), Path(id)).await), Some(StatusCode::CONFLICT));
}

#[tokio::test]
async fn a_chat_without_codex_threads_compacts_nothing() {
    let (state, id) = state();
    let Json(result) = compact_chat(State(state), Path(id)).await.ok().unwrap();
    assert_eq!((result.compacted, result.failed.len()), (0, 0));
}
