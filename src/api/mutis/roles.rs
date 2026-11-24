use crate::{
    api::{providers::{cached_provider_test, ConnectionTest}, ApiResult},
    app::AppState,
    config::{self, ProviderConfig},
};
use axum::{extract::{Query, State}, Json};
use futures_util::future::join_all;
use serde::Serialize;
use serde_json::Value;

#[derive(Serialize)]
pub(crate) struct ResolvedRole {
    configured_provider: Option<ProviderConfig>,
    live_status: Option<ConnectionTest>,
    used_provider: Option<ProviderConfig>,
}
#[derive(serde::Deserialize)]
pub(crate) struct ResolveQuery {
    /// The effort the user picked for MutiS; every effort runs the single High level.
    effort: Option<String>,
}
/// One role checked against the live sources. Keys are not masked: never send this to the browser.
pub(crate) struct RoleResolution {
    configured: Option<ProviderConfig>,
    status: Option<ConnectionTest>,
    used: Option<ProviderConfig>,
}

/// The source a chat at the "MutiS" source runs on, with the model and effort the lead role gives it.
pub(crate) struct Lead {
    pub provider: ProviderConfig,
    pub model: String,
    pub effort: String,
}

/// The lead's model comes from its role only when the role's own source is the one in use; a fallback source uses its first model.
/// A High level whose lead is not set up yet follows the Max roles, as the settings screen shows.
pub(super) fn pick_lead(
    cfg: &config::MutisConfig,
    _effort: &str,
    configured: Option<&ProviderConfig>,
    used: ProviderConfig,
) -> Lead {
    let roles = if cfg.high_roles.lead.provider_id.trim().is_empty() {
        &cfg.roles
    } else {
        &cfg.high_roles
    };
    let role = &roles.lead;
    let own = configured.is_some_and(|c| c.id == used.id) && !role.model.trim().is_empty();
    let model = if own {
        role.model.trim().to_owned()
    } else {
        used.models.first().cloned().unwrap_or_default()
    };
    Lead {
        provider: used,
        model,
        effort: role.effort.clone(),
    }
}

/// Resolves the lead role of the level `effort` picks. None when no source that could lead is connected.
pub(crate) async fn resolve_lead(state: &AppState, effort: &str) -> Option<Lead> {
    let cfg = state.config.read().await.mutis.clone();
    let (_, lead) = resolve_roles(state, effort).await.into_iter().next()?;
    let used = lead.used?;
    Some(pick_lead(&cfg, effort, lead.configured.as_ref(), used))
}

pub(crate) async fn resolve_mutis_roles(
    State(state): State<AppState>,
    Query(query): Query<ResolveQuery>,
) -> ApiResult<Json<Value>> {
    let effort = query.effort.as_deref().unwrap_or("high");
    // Never send raw API keys to the browser.
    let mask = |mut provider: ProviderConfig| {
        provider.api_key = config::masked_key(&provider.api_key);
        provider
    };
    let mut output = serde_json::Map::new();
    for (name, role) in resolve_roles(&state, effort).await {
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

/// The lead, big and small roles, in that order.
async fn resolve_roles(state: &AppState, effort: &str) -> Vec<(&'static str, RoleResolution)> {
    let cfg = state.config.read().await.clone();
    let roles = cfg.mutis.roles_for(effort);
    // High level never falls back to Claude: its whole point is to run without it.
    let high = config::MutisConfig::level_for(effort) == "high";
    let (lead_fallback, big_fallback, small_fallback): (&[&str], &[&str], &[&str]) = if high {
        (&["codex_cli"], &["codex_cli"], &["codex_cli"])
    } else {
        (
            &["claude_cli", "anthropic", "codex_cli"],
            &["codex_cli", "claude_cli", "anthropic"],
            &["codex_cli", "claude_cli"],
        )
    };
    let mut output = Vec::new();
    for (name, role, fallback) in [
        ("lead", &roles.lead, lead_fallback),
        ("big", &roles.big, big_fallback),
        ("small", &roles.small, small_fallback),
    ] {
        // The removed legacy agent id still resolves to the agent-capable Deploy source.
        let configured =
            crate::config::resolve_provider_id(&cfg.providers, &role.provider_id).cloned();
        let mut candidates = Vec::new();
        if let Some(provider) = &configured {
            candidates.push(provider.clone());
        }
        if name == "small" {
            for provider in cfg
                .providers
                .iter()
                .filter(|p| p.kind == "openai_compat" && p.agent_capable)
            {
                if !candidates.iter().any(|x| x.id == provider.id) {
                    candidates.push(provider.clone());
                }
            }
        }
        for kind in fallback {
            for provider in cfg.providers.iter().filter(|p| p.kind == *kind) {
                if !candidates.iter().any(|x| x.id == provider.id) {
                    candidates.push(provider.clone());
                }
            }
        }
        let results = join_all(
            candidates
                .iter()
                .map(|p| cached_provider_test(&state, &p.id)),
        )
        .await;
        let statuses = candidates
            .iter()
            .cloned()
            .zip(results.into_iter().map(Result::ok))
            .collect::<Vec<_>>();
        let configured_status = configured.as_ref().and_then(|c| {
            statuses
                .iter()
                .find(|(p, _)| p.id == c.id)
                .and_then(|(_, s)| s.clone())
        });
        let used = statuses
            .iter()
            .find(|(_, s)| s.as_ref().is_some_and(|s| s.status == "connected"))
            .map(|(p, _)| p.clone());
        output.push((
            name,
            RoleResolution {
                configured,
                status: configured_status,
                used,
            },
        ));
    }
    output
}
