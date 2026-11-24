use crate::{
    api::providers::cached_provider_test,
    app::AppState,
    chat,
    config::{Config, ProviderConfig},
};

#[derive(Clone)]
pub(super) struct Debater {
    pub(super) provider: ProviderConfig,
    pub(super) model: String,
    pub(super) effort: chat::Effort,
    pub(super) session: Option<String>,
}

/// The first connected CLI of `kind`, using its first model.
pub(super) async fn pick_debater(state: &AppState, cfg: &Config, kind: &str) -> Option<Debater> {
    for provider in cfg.providers.iter().filter(|provider| provider.kind == kind) {
        let connected = cached_provider_test(state, &provider.id)
            .await
            .is_ok_and(|status| status.status == "connected");
        if !connected {
            continue;
        }
        return Some(Debater {
            provider: provider.clone(),
            model: provider.models.first().cloned().unwrap_or_default(),
            effort: chat::Effort::Default,
            session: None,
        });
    }
    None
}
