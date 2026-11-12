//! "Test" button for an MCP server the app runs itself.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::{validate_mcp_servers, McpServerConfig},
    providers::mcp::McpClient,
};
use axum::{extract::State, http::StatusCode, Json};
use serde_json::{json, Value};

/// Starts the server once, lists its tools and stops it. Nothing is saved.
pub(crate) async fn test(State(state): State<AppState>, Json(server): Json<McpServerConfig>) -> ApiResult<Json<Value>> {
    validate_mcp_servers(std::slice::from_ref(&server))
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e.to_string()))?;
    let folder = state.config.read().await.default_folder.clone();
    let folder = if std::path::Path::new(&folder).is_dir() { folder } else { ".".to_owned() };
    let mut client = McpClient::start(&server, &folder)
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    let tools = client
        .list_tools()
        .await
        .map_err(|e| ApiError::new(StatusCode::BAD_GATEWAY, format!("{e:#}")))?;
    let names: Vec<Value> = tools
        .iter()
        .map(|tool| json!({"name": tool.name, "description": tool.description, "read_only": tool.read_only}))
        .collect();
    Ok(Json(json!({ "tools": names })))
}
