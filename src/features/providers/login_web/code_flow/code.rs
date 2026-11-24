use super::{
    cli_label,
    parse::{logged_in_email, push_tail, validate_login_code},
    LoginCodeOutcome,
};
use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
};
use std::time::{Duration, Instant};
use tokio::{io::AsyncWriteExt, time::timeout};

pub(crate) async fn login_provider_code(
    state: &AppState,
    id: &str,
    provider: &ProviderConfig,
    session_id: &str,
    code: &str,
) -> ProviderResult<LoginCodeOutcome> {
    let label = cli_label(&provider.kind);
    let code =
        validate_login_code(code).map_err(|e| ProviderError::new(ProviderErrorKind::BadRequest, e))?;
    let mut session = {
        let mut sessions = state.claude_login.lock().await;
        let session = sessions
            .as_mut()
            .filter(|s| s.id == session_id && s.provider_id == id)
            .ok_or_else(|| {
                ProviderError::new(
                    ProviderErrorKind::NotFound,
                    "Phiên đăng nhập đã hết hạn hoặc không tồn tại",
                )
            })?;
        session.last_activity = Instant::now();
        session
            .stdin
            .write_all(format!("{code}\n").as_bytes())
            .await
            .map_err(|e| {
                ProviderError::new(
                    ProviderErrorKind::BadGateway,
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
    Ok(LoginCodeOutcome {
        exited,
        message,
        email,
        output_tail,
    })
}
