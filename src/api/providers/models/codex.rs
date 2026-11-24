use super::catalog::ModelCatalog;
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::http::StatusCode;
use serde_json::Value;

/// Visible models and reasoning effort options from the signed-in Codex app-server.
pub(super) async fn codex_models(
    state: &AppState,
    provider: &crate::config::ProviderConfig,
) -> ApiResult<ModelCatalog> {
    let mut client = super::super::codex::new_client(state, provider).await?;
    let listed = list_codex_models(&mut client).await;
    client.close().await;
    let models = listed.map_err(|error| ApiError::new(StatusCode::BAD_GATEWAY, error.to_string()))?;
    if models.ids.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_GATEWAY,
            "Codex không trả model nào; hãy đăng nhập Codex rồi thử lại",
        ));
    }
    Ok(models)
}

async fn list_codex_models(client: &mut crate::providers::codex::transport::Client) -> anyhow::Result<ModelCatalog> {
    let mut models = ModelCatalog::default();
    let mut cursor: Option<String> = None;
    for _ in 0..20 {
        let mut params = serde_json::json!({"limit": 100, "includeHidden": false});
        if let Some(cursor) = &cursor {
            params["cursor"] = Value::String(cursor.clone());
        }
        let page = client.call("model/list", params).await?;
        for row in page["data"].as_array().into_iter().flatten() {
            models.add_codex(row);
        }
        cursor = page["nextCursor"].as_str().filter(|c| !c.is_empty()).map(str::to_owned);
        if cursor.is_none() {
            break;
        }
    }
    Ok(models)
}
