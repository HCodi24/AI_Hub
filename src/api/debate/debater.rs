use crate::{
    api::providers::cached_provider_test,
    app::AppState,
    chat,
    config::{Config, MutisRole, ProviderConfig},
};
use serde_json::json;

#[derive(Clone)]
pub(super) struct Debater {
    pub(super) provider: ProviderConfig,
    pub(super) model: String,
    pub(super) effort: chat::Effort,
    pub(super) session: Option<String>,
}

pub(super) fn parse_effort(value: &str) -> chat::Effort {
    serde_json::from_value(json!(value)).unwrap_or_default()
}

/// The connected CLI of `kind`, preferring the MutiS role's provider/model/effort when it is that kind.
pub(super) async fn pick_debater(
    state: &AppState,
    cfg: &Config,
    kind: &str,
    role: &MutisRole,
) -> Option<Debater> {
    let mut candidates: Vec<&ProviderConfig> = cfg
        .providers
        .iter()
        .filter(|p| p.kind == kind && p.id == role.provider_id)
        .collect();
    candidates.extend(
        cfg.providers
            .iter()
            .filter(|p| p.kind == kind && p.id != role.provider_id),
    );
    for provider in candidates {
        let connected = cached_provider_test(state, &provider.id)
            .await
            .is_ok_and(|status| status.status == "connected");
        if !connected {
            continue;
        }
        let from_role = provider.id == role.provider_id;
        let model = if from_role && !role.model.is_empty() {
            role.model.clone()
        } else {
            provider.models.first().cloned().unwrap_or_default()
        };
        let effort = if from_role {
            parse_effort(&role.effort)
        } else {
            chat::Effort::Default
        };
        return Some(Debater {
            provider: provider.clone(),
            model,
            effort,
            session: None,
        });
    }
    None
}
