use crate::{
    api::{providers::ConnectionTest, ApiResult},
    app::AppState,
    config::{self, ProviderConfig},
    mutis::resolve_roles,
};
use axum::{extract::State, Json};
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub(crate) struct ResolvedRole {
    configured_provider: Option<ProviderConfig>,
    live_status: Option<ConnectionTest>,
    used_provider: Option<ProviderConfig>,
}
pub(crate) async fn resolve_mutis_roles(
    State(state): State<AppState>,
) -> ApiResult<Json<Value>> {
    // Never send raw API keys to the browser.
    let mask = |mut provider: ProviderConfig| {
        provider.api_key = config::masked_key(&provider.api_key);
        provider
    };
    let mut output = serde_json::Map::new();
    for (name, role) in resolve_roles(&state).await {
        output.insert(
            name.into(),
            serde_json::to_value(ResolvedRole {
                configured_provider: role.configured.map(mask),
                live_status: role.status,
                used_provider: role.used.map(mask),
            })
            .unwrap_or(Value::Null),
        );
    }
    Ok(Json(Value::Object(output)))
}
