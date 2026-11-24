use crate::{
    api::providers::{cached_provider_test, ConnectionTest},
    app::AppState,
    config::{self, Config, ProviderConfig},
};

/// One role checked against the live sources. Keys are not masked: never send this to the browser.
pub(crate) struct RoleResolution {
    pub(crate) configured: Option<ProviderConfig>,
    pub(crate) status: Option<ConnectionTest>,
    pub(crate) used: Option<ProviderConfig>,
}

/// The source a chat at the "MutiS" source runs on, with the model and effort the lead role gives it.
pub(crate) struct Lead {
    pub provider: ProviderConfig,
    pub model: String,
    pub effort: String,
}

/// Resolves the configured lead when its source is connected.
pub(crate) async fn resolve_lead(state: &AppState) -> Option<Lead> {
    let (lead, configured) = {
        let config = state.config.read().await;
        let lead = config.mutis.lead.clone();
        let configured = configured_provider(&config, &lead.provider_id);
        (lead, configured)
    };
    let provider = resolve_role(state, configured).await.used?;
    let model = if lead.model.trim().is_empty() {
        provider.models.first().cloned().unwrap_or_default()
    } else {
        lead.model
    };
    Some(Lead {
        provider,
        model,
        effort: lead.effort,
    })
}

/// The lead, worker and checker roles, in that order.
pub(crate) async fn resolve_roles(state: &AppState) -> [(&'static str, RoleResolution); 3] {
    let [lead, worker, checker] = {
        let config = state.config.read().await;
        [
            &config.mutis.lead.provider_id,
            &config.mutis.worker.provider_id,
            &config.mutis.checker.provider_id,
        ]
        .map(|id| configured_provider(&config, id))
    };
    [
        ("lead", resolve_role(state, lead).await),
        ("worker", resolve_role(state, worker).await),
        ("checker", resolve_role(state, checker).await),
    ]
}

fn configured_provider(config: &Config, id: &str) -> Option<ProviderConfig> {
    if id.is_empty() {
        return None;
    }
    // The removed legacy agent id still resolves to the agent-capable Deploy source.
    config::resolve_provider_id(&config.providers, id).cloned()
}

async fn resolve_role(state: &AppState, configured: Option<ProviderConfig>) -> RoleResolution {
    let status = match &configured {
        Some(provider) => cached_provider_test(state, &provider.id).await.ok(),
        None => None,
    };
    let used = if status.as_ref().is_some_and(|test| test.status == "connected") {
        configured.clone()
    } else {
        None
    };
    RoleResolution {
        configured,
        status,
        used,
    }
}
