use super::{
    cli_label, login_provider_config,
    parse::{logged_in_email, push_tail, validate_login_code},
    LoginCodeInput, LoginCodeResponse,
};
use super::super::{
    antigravity::EMAIL_FILE,
    connection::{connection, test_provider_uncached},
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    providers,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::time::{Duration, Instant};
use tokio::{io::AsyncWriteExt, time::timeout};

pub(crate) async fn login_provider_code(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<LoginCodeInput>,
) -> ApiResult<Json<LoginCodeResponse>> {
    let provider = login_provider_config(&state, &id).await?;
    let label = cli_label(&provider.kind);
    let code =
        validate_login_code(&input.code).map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, e))?;
    let mut session = {
        let mut sessions = state.claude_login.lock().await;
        let session = sessions
            .as_mut()
            .filter(|s| s.id == input.session_id && s.provider_id == id)
            .ok_or_else(|| {
                ApiError::new(
                    StatusCode::NOT_FOUND,
                    "Phiên đăng nhập đã hết hạn hoặc không tồn tại",
                )
            })?;
        session.last_activity = Instant::now();
        session
            .stdin
            .write_all(format!("{code}\n").as_bytes())
            .await
            .map_err(|e| {
                ApiError::new(
                    StatusCode::BAD_GATEWAY,
                    format!("Không gửi được mã xác nhận: {e}"),
                )
            })?;
        sessions.take().expect("session present")
    };
    let exit = timeout(Duration::from_secs(30), session.child.wait()).await;
    let (exited, message) = match &exit {
        Ok(Ok(status)) => (
            true,
            if status.success() {
                format!("{label} đã hoàn tất đăng nhập")
            } else {
                format!("{label} kết thúc với mã {}", status.code().unwrap_or(-1))
            },
        ),
        Ok(Err(e)) => (true, format!("Không chờ được {label}: {e}")),
        Err(_) => (false, format!("{label} chưa kết thúc sau 30 giây")),
    };
    while let Ok(line) = session.stdout.try_recv() {
        push_tail(&mut session.output_tail, &line);
    }
    let email = logged_in_email(&session.output_tail);
    let mut lines = session.output_tail.clone();
    lines.extend(session.stderr_tail.lock().await.iter().cloned());
    let output_tail = lines
        .into_iter()
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
        .replace(&code, "[đã ẩn mã]");
    let still_running =
        !matches!(exit, Ok(Ok(_))) && !matches!(session.child.try_wait(), Ok(Some(_)));
    if still_running {
        let mut sessions = state.claude_login.lock().await;
        if sessions.is_none() {
            *sessions = Some(session);
        }
    } else {
        drop(session);
    }
    let status = if exited {
        state.provider_status.lock().await.remove(&id);
        test_provider_uncached(State(state.clone()), Path(id))
            .await?
            .0
    } else {
        connection(false, "error", &message, "")
    };
    let logged_in = status.status == "connected";
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
