//! High roles: Codex leads and handles code; DeepSeek handles small tasks.
use super::Config;

pub(super) fn migrate_roles(config: &mut Config) {
    if [
        &config.mutis.high_roles.lead,
        &config.mutis.high_roles.big,
        &config.mutis.high_roles.small,
    ]
    .iter()
    .all(|r| r.provider_id.is_empty())
    {
        let codex = config.providers.iter().find(|p| p.kind == "codex_cli");
        let deepseek = config
            .providers
            .iter()
            .find(|p| p.id == config.mutis.roles.small.provider_id)
            .or_else(|| {
                config.providers.iter().find(|p| {
                    [&p.id, &p.name]
                        .iter()
                        .any(|s| s.to_ascii_lowercase().contains("deepseek"))
                        || p.models
                            .iter()
                            .any(|s| s.to_ascii_lowercase().contains("deepseek"))
                })
            });
        if let Some(codex) = codex {
            let codex_role = || super::MutisRole {
                provider_id: codex.id.clone(),
                model: "gpt-6.1-sol".into(),
                effort: "medium".into(),
            };
            let small = deepseek
                .map(|p| super::MutisRole {
                    provider_id: p.id.clone(),
                    model: p.models.first().cloned().unwrap_or_default(),
                    effort: "max".into(),
                })
                .unwrap_or_default();
            config.mutis.high_roles = super::MutisRoles {
                lead: codex_role(),
                big: codex_role(),
                small,
            };
        }
    }
}
