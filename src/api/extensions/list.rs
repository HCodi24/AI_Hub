use crate::app::AppState;
use axum::{extract::{Path, State}, Json};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
pub(crate) struct ExtensionQuery {
    folder: Option<String>,
}
#[derive(Serialize)]
pub(crate) struct StopResponse {
    stopped: bool,
}

pub(crate) async fn extension_list(
    axum::extract::Query(q): axum::extract::Query<ExtensionQuery>,
) -> Json<Vec<crate::extensions::Card>> {
    Json(crate::extensions::status(q.folder.as_deref()).await)
}
pub(crate) async fn extension_stop(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Json<StopResponse> {
    let token = state.extension_running.lock().await.get(&id).cloned();
    if let Some(token) = token {
        token.cancel();
        Json(StopResponse { stopped: true })
    } else {
        Json(StopResponse { stopped: false })
    }
}
