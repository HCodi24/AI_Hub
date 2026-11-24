use super::super::{
    connection::{connection, ConnectionTest},
    shared::find_provider,
};
use super::{error, new_client, sandbox_status};
use crate::{
    api::ApiResult,
    app::AppState,
    providers::{cli_support, codex::home},
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

#[derive(Serialize)]
pub(crate) struct Status {
    pub(super) connected: bool,
    pub(super) pending: bool,
    url: Option<String>,
    message: String,
    pub(super) project_skills: bool,
    pub(super) project_instructions: bool,
    /// Windows sandbox of this MutiS Codex home: `ready`, `notConfigured`, `updateRequired` or `unknown`.
    sandbox: String,
    /// Signed-in account of this source; `origin` is `cli` (copied from this machine) or `web`.
    email: Option<String>,
    plan: Option<String>,
    origin: Option<String>,
}

/// `account/read` result: `{ account: { type, email?, planType? } }`; never contains tokens.
fn account_text(account: &Value, key: &str) -> Option<String> {
    account["account"][key]
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

fn connected(account: &Value) -> bool {
    matches!(
        account["account"]["type"].as_str(),
        Some("chatgpt" | "apiKey")
    )
}

pub(crate) async fn codex_status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Status>> {
    let provider =
        find_provider(&state, &id, Some("codex_cli"), "Không tìm thấy nguồn Codex").await?;
    let mut connections = state.codex_connections.lock().await;
    if let Some(session) = connections.get(&id) {
        if session.started.elapsed() > Duration::from_secs(600) {
            if let Some(mut session) = connections.remove(&id) {
                session.client.close().await;
            }
        }
    }
    let mut temporary_account = None;
    if !connections.contains_key(&id) {
        // No credentials: avoid starting a process just to show the signed-out state.
        let dir = home(&state.config_path, &provider).map_err(error)?;
        if !dir.join("auth.json").is_file() {
            let config = state.config.read().await;
            return Ok(Json(Status {
                connected: false,
                pending: false,
                url: None,
                message: "Chưa đăng nhập Codex trong MutiS".into(),
                project_skills: config.codex.project_skills,
                project_instructions: config.codex.project_instructions,
                sandbox: "unknown".into(),
                email: None,
                plan: None,
                origin: None,
            }));
        }
        drop(connections);
        let mut temporary = new_client(&state, &provider).await?;
        let result = temporary
            .call("account/read", json!({"refreshToken":false}))
            .await;
        let sandbox = sandbox_status(&mut temporary).await;
        temporary.close().await;
        temporary_account = Some((result.map_err(error)?, sandbox));
        connections = state.codex_connections.lock().await;
    }
    // Prefer a live session that appeared while the temporary client was running.
    let (account, sandbox) = if let Some(session) = connections.get_mut(&id) {
        let account = session
            .client
            .call("account/read", json!({"refreshToken":false}))
            .await
            .map_err(error)?;
        let sandbox = sandbox_status(&mut session.client).await;
        (account, sandbox)
    } else {
        temporary_account.expect("temporary account read without a live session")
    };
    let is_connected = connected(&account);
    let failed = connections
        .get_mut(&id)
        .is_some_and(|s| s.client.login_failed());
    if is_connected || failed {
        if let Some(mut session) = connections.remove(&id) {
            session.client.close().await;
        }
    }
    let pending = connections.get(&id).is_some_and(|s| s.login_id.is_some());
    let url = connections.get(&id).and_then(|s| s.url.clone());
    drop(connections);
    let origin = if is_connected {
        home(&state.config_path, &provider)
            .ok()
            .and_then(|dir| cli_support::auth_origin(&dir))
    } else {
        None
    };
    let email = is_connected
        .then(|| account_text(&account, "email"))
        .flatten();
    let message = if is_connected {
        cli_support::connected_message(email.as_deref(), origin.as_deref())
    } else if pending {
        "Đang chờ đăng nhập trên trình duyệt".to_owned()
    } else {
        "Chưa đăng nhập Codex trong MutiS".to_owned()
    };
    let config = state.config.read().await;
    let status = Status {
        connected: is_connected,
        pending,
        url,
        message: message.clone(),
        project_skills: config.codex.project_skills,
        project_instructions: config.codex.project_instructions,
        sandbox,
        email,
        plan: is_connected
            .then(|| account_text(&account, "planType"))
            .flatten(),
        origin,
    };
    drop(config);
    state.provider_status.lock().await.insert(
        id,
        (
            Instant::now(),
            connection(
                is_connected,
                if is_connected {
                    "connected"
                } else {
                    "not_configured"
                },
                &message,
                "",
            ),
        ),
    );
    Ok(Json(status))
}

pub(in crate::api::providers) async fn test_connection(state: &AppState, id: &str) -> ConnectionTest {
    match codex_status(State(state.clone()), Path(id.to_owned())).await {
        Ok(Json(status)) => connection(
            status.connected,
            if status.connected {
                "connected"
            } else {
                "not_configured"
            },
            &status.message,
            "",
        ),
        Err(_) => connection(
            false,
            "error",
            "Không kiểm tra được Codex trong MutiS",
            "Kiểm tra bản cài và đăng nhập",
        ),
    }
}
