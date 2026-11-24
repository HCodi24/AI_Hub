use super::folder::project_folder;
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
use std::collections::BTreeMap;

pub(super) const MAX_PROJECT_COMMANDS: usize = 200;

#[derive(Deserialize)]
pub(crate) struct CommandsQuery {
    folder: String,
}
#[derive(Deserialize)]
pub(crate) struct CommandsInput {
    folder: String,
    command: String,
}

pub(super) fn set_command(
    commands: &mut BTreeMap<String, String>,
    key: String,
    command: String,
) -> Result<(), &'static str> {
    if command.is_empty() {
        commands.remove(&key);
    } else if commands.contains_key(&key) || commands.len() < MAX_PROJECT_COMMANDS {
        commands.insert(key, command);
    } else {
        return Err("Đã lưu quá nhiều dự án; xóa bớt trước");
    }
    Ok(())
}

/// What the user needs to confirm a project's verify command: the saved one, the global one and what auto-detection would run.
async fn commands_view(state: &AppState, folder: &std::path::Path) -> Value {
    let config = state.config.read().await;
    json!({
        "folder": folder.to_string_lossy(),
        "saved": config.mutis.verify_commands.get(&crate::verify::folder_key(folder)),
        "global": config.mutis.verify_command,
        "detected": crate::verify::detected_commands(folder).into_iter().map(|(name, command)| json!({"name": name, "command": command})).collect::<Vec<_>>(),
    })
}

pub(crate) async fn verify_commands_get(State(state): State<AppState>, Query(query): Query<CommandsQuery>) -> ApiResult<Json<Value>> {
    let folder = project_folder(&query.folder)?;
    Ok(Json(commands_view(&state, &folder).await))
}

/// Saves (or, when empty, removes) the verify command of one project folder.
pub(crate) async fn verify_commands_put(State(state): State<AppState>, Json(input): Json<CommandsInput>) -> ApiResult<Json<Value>> {
    let folder = project_folder(&input.folder)?;
    let command = input.command.trim().to_owned();
    if command.chars().count() > 2000 {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "Lệnh kiểm tra dài quá 2000 ký tự"));
    }
    {
        let mut config = state.config.write().await;
        let mut next = config.clone();
        let key = crate::verify::folder_key(&folder);
        set_command(&mut next.mutis.verify_commands, key, command)
            .map_err(|message| ApiError::new(StatusCode::BAD_REQUEST, message))?;
        crate::config::save(&state.config_path, &next)
            .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được cài đặt"))?;
        *config = next;
    }
    Ok(Json(commands_view(&state, &folder).await))
}
