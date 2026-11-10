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
fn store(state: &AppState, folder: &Folder) -> ApiResult<Store> {
    Store::open(&state.config_path, &folder.folder).map_err(error)
}
pub(crate) async fn list(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
) -> ApiResult<Json<Value>> {
    let view = store(&state, &folder)?.overview().map_err(error)?;
    let summaries = |scopes: &[crate::features::memory::Scope]| {
        scopes
            .iter()
            .map(|s| {
                json!({
                    "scope": s.scope, "title": s.title, "status": s.status,
                    "updated": s.updated, "next": s.next,
                })
            })
            .collect::<Vec<_>>()
    };
    Ok(Json(json!({"project":view.project,"index":view.index,
        "scopes":summaries(&view.scopes),"archived":summaries(&view.archived)})))
}
pub(crate) async fn get(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        json!({"text":store(&state,&folder)?.read(&id).map_err(error)?}),
    ))
}
pub(crate) async fn put(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
    Json(edit): Json<Edit>,
) -> ApiResult<Json<Value>> {
    let config = state.config.read().await.memory.clone();
    Ok(Json(
        json!({"text":store(&state,&folder)?.edit(&id,&edit.text,&config).map_err(error)?}),
    ))
}
pub(crate) async fn archive(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder)?.archive(&id, false).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
pub(crate) async fn restore(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder)?.archive(&id, true).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
pub(crate) async fn delete(
    State(state): State<AppState>,
    Query(folder): Query<Folder>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    store(&state, &folder)?.delete(&id).map_err(error)?;
    Ok(Json(json!({"ok":true})))
}
