use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRecord},
    config::{MutisConfig, ProviderConfig},
    providers,
};
use axum::http::StatusCode;

pub(super) type DelegateRole = (ProviderConfig, String, chat::Effort);

/// Kinds of source that can run delegated tasks: an API agent loop (OpenAI-compatible) or a CLI (Codex, Claude, Antigravity).
const WORKER_KINDS: [&str; 4] = ["openai_compat", "codex_cli", "claude_cli", "antigravity_cli"];

/// Sources the delegated worker may be, best first: the role's own source, then the agent-capable API sources, then the Codex sources.
/// The caller uses the first one that is active (connected).
pub(super) fn worker_candidates(
    providers: &[crate::config::ProviderConfig],
    configured_id: &str,
) -> Vec<crate::config::ProviderConfig> {
    let mut found: Vec<_> = crate::config::resolve_provider_id(providers, configured_id)
        .filter(|p| WORKER_KINDS.contains(&p.kind.as_str()))
        .cloned()
        .into_iter()
        .collect();
    for provider in providers {
        let fits = (provider.kind == "openai_compat" && provider.agent_capable)
            || provider.kind == "codex_cli";
        if fits && !found.iter().any(|p| p.id == provider.id) {
            found.push(provider.clone());
        }
    }
    found
}

pub(super) async fn select_delegate(
    state: &AppState,
    lead_kind: &str,
    is_mutis: bool,
    is_plan_task: bool,
    plain_chat: bool,
    mutis_config: &MutisConfig,
    role_effort_name: &str,
) -> (Option<DelegateRole>, Option<DelegateRole>, Option<String>) {
    // MutiS High + Codex: the app runs the delegated worker for Codex (see providers::delegate). Any source that is
    // active in Settings › Nguồn AI can be the worker, whatever its kind (API key or CLI).
    let delegate_worker = if matches!(lead_kind, "codex_cli" | "claude_cli")
        && is_mutis
        && !is_plan_task
        && !plain_chat
        && crate::config::MutisConfig::level_for(role_effort_name) == "high"
    {
        let providers_now = state.config.read().await.providers.clone();
        // High roles may be unset (old configs): fall back to the Max small worker.
        let high_small = &mutis_config.roles_for(role_effort_name).small;
        let role = if high_small.provider_id.trim().is_empty() {
            &mutis_config.roles.small
        } else {
            high_small
        };
        let configured_id = crate::config::resolve_provider_id(&providers_now, &role.provider_id)
            .map(|p| p.id.clone());
        let mut chosen = None;
        for provider in worker_candidates(&providers_now, &role.provider_id) {
            let active = crate::api::providers::cached_provider_test(state, &provider.id)
                .await
                .is_ok_and(|test| test.status == "connected");
            let own_model = configured_id.as_deref() == Some(provider.id.as_str())
                && !role.model.trim().is_empty();
            let model = if own_model {
                role.model.trim().to_owned()
            } else {
                provider.models.first().cloned().unwrap_or_default()
            };
            if active && !model.is_empty() {
                chosen = Some((
                    provider,
                    model,
                    chat::parse_effort(&role.effort).unwrap_or_default(),
                ));
                break;
            }
        }
        chosen
    } else {
        None
    };
    // The optional independent checker: the High section's "checker" role, any source (a thread is resumed only where the source keeps one).
    let (delegate_checker, checker_note) = if delegate_worker.is_some()
        && mutis_config.delegate_checker
    {
        let role = &mutis_config.high_checker;
        let config = state.config.read().await;
        let found = crate::config::resolve_provider_id(&config.providers, &role.provider_id)
            .and_then(|p| {
                let model = if role.model.trim().is_empty() {
                    p.models.first().cloned().unwrap_or_default()
                } else {
                    role.model.trim().to_owned()
                };
                (!model.is_empty()).then(|| {
                    (
                        p.clone(),
                        model,
                        chat::parse_effort(&role.effort).unwrap_or(chat::Effort::Max),
                    )
                })
            });
        let note = found.is_none().then(|| {
            "Người kiểm tra chưa chạy: vào Cài đặt › MutiS › Mức High, mục «Người kiểm tra», chọn nguồn và model."
                .to_owned()
        });
        (found, note)
    } else {
        (None, None)
    };
    (delegate_worker, delegate_checker, checker_note)
}

pub(super) async fn build_provider(
    state: &AppState,
    lead_kind: &str,
    chat_id: &str,
    folder: &str,
    existing: Option<&ChatRecord>,
    delegate_worker: Option<&DelegateRole>,
    delegate_checker: &Option<DelegateRole>,
    checker_note: &Option<String>,
) -> ApiResult<Box<dyn providers::Provider>> {
    let provider: Box<dyn providers::Provider> = match delegate_worker {
        Some((worker, model, effort)) => {
            // A Codex worker's thread only resumes inside the Codex home it was made in.
            let kept = |id: &str| existing.and_then(|r| r.worker_sessions.get(id).cloned());
            let mut worker_session = kept(&worker.id);
            let mut worker2_session = kept(&providers::delegate::worker2_key(&worker.id));
            if worker.kind == "codex_cli" {
                let settings = state.config.read().await.codex.clone();
                let prefix = providers::codex::session_prefix(&settings, worker, folder);
                worker_session = worker_session.filter(|id| id.starts_with(&prefix));
                worker2_session = worker2_session.filter(|id| id.starts_with(&prefix));
            }
            Box::new(providers::delegate::DelegateProvider {
                worker: worker.clone(),
                model: model.clone(),
                effort: *effort,
                checker: delegate_checker.clone(),
                checker_note: checker_note.clone(),
                chat_id: chat_id.to_owned(),
                lead_kind: lead_kind.to_owned(),
                worker_session,
                worker2_session,
            })
        }
        None => providers::for_kind(lead_kind).map_err(|_| {
            ApiError::new(StatusCode::BAD_REQUEST, "Loại nhà cung cấp không hợp lệ")
        })?,
    };
    Ok(provider)
}
