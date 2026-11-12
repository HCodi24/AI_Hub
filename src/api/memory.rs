use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::memory::Store,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub(crate) struct Folder {
    folder: String,
}
#[derive(Deserialize)]
pub(crate) struct Edit {
    text: String,
}
fn error(e: impl std::fmt::Display) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, e.to_string())
}
async fn store(state: &AppState, folder: &Folder) -> ApiResult<Store> {
    let config = state.config.read().await.memory.clone();
    Store::open(&state.config_path, &folder.folder, &config).map_err(error)
}
pub(crate) async fn list(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
) -> ApiResult<Json<Value>> {
    let view = store(&state, &folder).await?.overview().map_err(error)?;
    let summaries = |scopes: &[crate::features::memory::Session]| {
        scopes
            .iter()
            .map(|s| {
                json!({
                    "scope": s.id, "title": s.title, "status": s.status,
                    "updated": s.updated, "next": s.open.first().cloned().unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>()
    };
    Ok(Json(json!({"project":view.project,"index":view.core,
        "scopes":summaries(&view.sessions),"archived":summaries(&view.archived)})))
}
pub(crate) async fn get(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"text":store(&state,&folder).await?.read(&id).map_err(error)?}),
    ))
}
pub(crate) async fn put(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
    Json(edit): Json<Edit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"text":store(&state,&folder).await?.edit(&id,&edit.text).map_err(error)?}),
    ))
}
pub(crate) async fn archive(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder).await?.archive(&id, false).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
pub(crate) async fn restore(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder).await?.archive(&id, true).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
pub(crate) async fn delete(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder).await?.delete(&id).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub(crate) struct Recall {
    folder: Option<String>,
    session: Option<String>,
    project: Option<String>,
}
/// Projects known to the memory (`projects.txt`).
pub(crate) async fn projects(State(state): State<AppState>) -> Json<Value> {
    Json(json!({"projects": crate::features::memory::list_projects(&state.config_path)}))
}
/// Text the UI attaches to the next message: one session of this folder, or another project's core.
pub(crate) async fn recall(
    State(state): State<AppState>,
    Query(q): Query<Recall>,
) -> ApiResult<Json<Value>> {
    let text = match (q.folder, q.session, q.project) {
        (Some(folder), Some(id), None) => store(&state, &Folder { folder })
            .await?
            .recall(&id)
            .map_err(error)?,
        (None, None, Some(slug)) => {
            crate::features::memory::project_core(&state.config_path, &slug).map_err(error)?
        }
        _ => return Err(error("Use folder+session or project")),
    };
    Ok(Json(json!({"text": text})))
}
