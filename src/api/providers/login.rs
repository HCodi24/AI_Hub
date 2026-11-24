use super::{
    claude_error, ensure_claude_idle, ensure_login_mode, invalidate_claude, test_provider_uncached,
    EMAIL_FILE,
};
use crate::{
    api::ApiResult,
    app::AppState,
    features::providers::login_web::code_flow::{self, LoginCodeOutcome, LoginStartResponse},
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::{sleep, timeout};

#[derive(Deserialize)]
pub(crate) struct LoginCodeInput {
    session_id: String,
    code: String,
}
#[derive(Serialize)]
pub(crate) struct LoginCodeResponse {
    ok: bool,
    logged_in: bool,
    message: String,
    output_tail: String,
}

pub(crate) async fn login_provider_start(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<LoginStartResponse>> {
    let provider = code_flow::login_provider_config(&state, &id).await?;
    let agy = provider.kind == "antigravity_cli";
    let dir = if agy {
        providers::antigravity_cli::clean_home(&provider.id).map_err(claude_error)?
    } else {
        ensure_login_mode(&provider, true)?;
        ensure_claude_idle(&state).await?;
        let dir = providers::claude::environment::home(&state.config_path, &provider)
            .map_err(claude_error)?;
        invalidate_claude(&state).await?;
        dir
    };
    let result = code_flow::login_provider_start(&state, &id, &provider, &dir).await?;
    state.provider_status.lock().await.remove(&id);
    if !agy {
        let session_id = result.session_id.clone();
        tokio::spawn(async move {
            let exited = timeout(Duration::from_secs(10 * 60), async {
                loop {
                    sleep(Duration::from_secs(1)).await;
                    let mut active = state.claude_login.lock().await;
                    let Some(session) = active
                        .as_mut()
                        .filter(|session| session.id == session_id && session.provider_id == id)
                    else {
                        return false;
                    };
                    match session.child.try_wait() {
                        Ok(Some(_)) => {
                            drop(active.take());
                            return true;
                        }
                        Ok(None) => {}
                        Err(_) => return false,
                    }
                }
            })
            .await;
            if !matches!(exited, Ok(true)) {
                return;
            }

            state.provider_status.lock().await.remove(&id);
            if let Ok(Json(status)) = test_provider_uncached(State(state.clone()), Path(id)).await {
                if status.status == "connected" {
                    if let Ok(home) =
                        providers::claude::environment::home(&state.config_path, &provider)
                    {
                        providers::cli_support::set_auth_origin(&home, Some("web"));
                    }
                }
            }
        });
    }
    Ok(Json(result))
}

pub(crate) async fn login_provider_code(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<LoginCodeInput>,
) -> ApiResult<Json<LoginCodeResponse>> {
    let provider = code_flow::login_provider_config(&state, &id).await?;
    let LoginCodeOutcome {
        exited,
        message,
        email,
        output_tail,
    } = code_flow::login_provider_code(&state, &id, &provider, &input.session_id, &input.code).await?;
    let logged_in = if exited {
        state.provider_status.lock().await.remove(&id);
        test_provider_uncached(State(state.clone()), Path(id))
            .await?
            .0
            .status
            == "connected"
    } else {
        false
    };
    if logged_in && provider.kind == "antigravity_cli" {
        if let (Some(email), Ok(home)) =
            (email, providers::antigravity_cli::clean_home(&provider.id))
        {
            let _ = std::fs::write(home.join(EMAIL_FILE), email);
            // The status was read before the email was saved; drop it so the next read shows the account.
            state.provider_status.lock().await.remove(&provider.id);
        }
    } else if logged_in {
        if let Ok(home) = providers::claude::environment::home(&state.config_path, &provider) {
            providers::cli_support::set_auth_origin(&home, Some("web"));
        }
    }
    Ok(Json(LoginCodeResponse {
        ok: exited && logged_in,
        logged_in,
        message: if logged_in && provider.kind == "antigravity_cli" {
            "Đã đăng nhập Antigravity".into()
        } else if logged_in {
            "Đã đăng nhập Claude Code CLI".into()
        } else {
            message
        },
        output_tail,
    }))
}

pub(crate) async fn login_provider_cancel(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    code_flow::login_provider_cancel(&state, &id).await?;
    Ok(StatusCode::NO_CONTENT)
}
