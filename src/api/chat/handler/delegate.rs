use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    chat::{self, ChatRecord},
    config::{MutisConfig, ProviderConfig},
    providers,
};
use axum::http::StatusCode;

pub(super) type DelegateRole = (ProviderConfig, String, chat::Effort);

pub(super) async fn select_delegate(
    state: &AppState,
    is_mutis: bool,
    is_plan_task: bool,
    plain_chat: bool,
    mutis_config: &MutisConfig,
) -> (Option<DelegateRole>, Option<DelegateRole>, Option<String>) {
    if !is_mutis || is_plan_task || plain_chat {
        return (None, None, None);
    }

    let worker = &mutis_config.worker;
    let delegate_worker = {
        let config = state.config.read().await;
        configured_role(&config.providers, &worker.provider_id, &worker.model, &worker.effort)
    };
    let Some(worker) = delegate_worker else {
        return (None, None, None);
    };
    // Only a signed-out worker is dropped: a test that merely failed (network timeout) must not silently turn delegation
    // off for this turn; a worker that is really broken reports its own error in the round.
    let signed_out = crate::api::providers::cached_provider_test(state, &worker.0.id)
        .await
        .is_ok_and(|test| test.status == "not_configured");
    if signed_out {
        return (None, None, None);
    }

    let (delegate_checker, checker_note) = if mutis_config.delegate_checker {
        let checker = &mutis_config.checker;
        let config = state.config.read().await;
        let found = configured_role(
            &config.providers,
            &checker.provider_id,
            &checker.model,
            &checker.effort,
        );
        let note = found.is_none().then(|| {
            "Người kiểm tra chưa chạy: vào Cài đặt › MutiS › Người kiểm tra, chọn nguồn và model."
                .to_owned()
        });
        (found, note)
    } else {
        (None, None)
    };
    (Some(worker), delegate_checker, checker_note)
}

fn configured_role(
    providers: &[ProviderConfig],
    provider_id: &str,
    model: &str,
    effort: &str,
) -> Option<DelegateRole> {
    if provider_id.is_empty() {
        return None;
    }
    let provider = crate::config::resolve_provider_id(providers, provider_id)?;
    let model = if model.trim().is_empty() {
        provider.models.first().cloned().unwrap_or_default()
    } else {
        model.trim().to_owned()
    };
    if model.trim().is_empty() {
        return None;
    }
    Some((
        provider.clone(),
        model,
        chat::parse_effort(effort).unwrap_or_default(),
    ))
}

fn role_permission(value: &str) -> chat::Permission {
    match value {
        "read" => chat::Permission::Read,
        "write" => chat::Permission::Write,
        _ => chat::Permission::Full,
    }
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
            let config = state.config.read().await;
            // A Codex worker's thread only resumes inside the Codex home it was made in.
            let prefix = (worker.kind == "codex_cli")
                .then(|| providers::codex::session_prefix(&config.codex, worker, folder));
            let worker_sessions = (0..providers::delegate::WORKER_SLOTS)
                .map(|slot| {
                    let key = providers::delegate::slot_key(&worker.id, slot);
                    existing
                        .and_then(|record| record.worker_sessions.get(&key))
                        .filter(|id| prefix.as_deref().is_none_or(|prefix| id.starts_with(prefix)))
                        .cloned()
                })
                .collect();
            Box::new(providers::delegate::DelegateProvider {
                worker: worker.clone(),
                model: model.clone(),
                effort: *effort,
                worker_permission: role_permission(&config.mutis.worker.permission),
                worker_count: config.mutis.worker.count.clamp(1, 3) as usize,
                checker: delegate_checker.clone(),
                checker_permission: role_permission(&config.mutis.checker.permission),
                checker_note: checker_note.clone(),
                chat_id: chat_id.to_owned(),
                lead_kind: lead_kind.to_owned(),
                worker_sessions,
            })
        }
        None => providers::for_kind(lead_kind).map_err(|_| {
            ApiError::new(StatusCode::BAD_REQUEST, "Loại nhà cung cấp không hợp lệ")
        })?,
    };
    Ok(provider)
}
