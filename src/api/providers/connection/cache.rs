use super::{test_provider_uncached, ConnectionTest};
use crate::{api::ApiResult, app::AppState};
use axum::{
    extract::{Path, State},
    Json,
};
use std::time::{Duration, Instant};

pub(crate) async fn cached_provider_test(state: &AppState, id: &str) -> ApiResult<ConnectionTest> {
    if let Some((checked, status)) = state.provider_status.lock().await.get(id).cloned() {
        if checked.elapsed() < Duration::from_secs(60) {
            return Ok(status);
        }
    }
    let status = test_provider_uncached(State(state.clone()), Path(id.to_owned()))
        .await?
        .0;
    state
        .provider_status
        .lock()
        .await
        .insert(id.to_owned(), (Instant::now(), status.clone()));
    Ok(status)
}

pub(crate) async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    if let Some(session) = state.claude_login.lock().await.as_mut() {
        if session.provider_id == id {
            session.last_activity = Instant::now();
        }
    }
    state.provider_status.lock().await.remove(&id);
    cached_provider_test(&state, &id).await.map(Json)
}
