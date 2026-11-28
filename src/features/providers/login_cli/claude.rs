use crate::{
    config::ProviderConfig,
    features::providers::{
        cli::claude::{
            import_cli_account, login_machine_account, provider_error, LOGIN_BUSY_MESSAGE,
            LOGIN_FAILED_MESSAGE, LOGIN_TIMEOUT_MESSAGE,
        },
        error::{ProviderError, ProviderErrorKind, ProviderResult},
        login_mode::ensure_login_mode,
    },
    providers::cli_support,
};
use std::path::Path;

/// Messages written by the import itself; they tell the user what to do, so show them as they are.
const OWN_MESSAGES: &[&str] = &[
    "Không có đăng nhập Claude",
    "Claude CLI chưa có đăng nhập",
    "Claude CLI trên máy chưa đăng nhập",
    "Đăng nhập Claude không hợp lệ",
    "Không tìm thấy cấu hình Claude CLI",
];

pub(crate) async fn connect_machine_account(
    provider: &ProviderConfig,
    home: &Path,
) -> ProviderResult<()> {
    ensure_login_mode(provider, false)?;
    let executable = cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::NotFound, "Chưa tìm thấy Claude CLI trên máy")
        })?;
    match import_cli_account(&executable, home).await {
        // This machine's Claude CLI is not signed in: sign it in on the browser, then copy the account.
        Err(error) if needs_machine_login(&error) => {
            eprintln!("Claude import: {error:#}; starting claude auth login");
            login_machine_account(&executable).await.map_err(import_error)?;
            import_cli_account(&executable, home).await.map_err(import_error)
        }
        result => result.map_err(import_error),
    }
}

/// Import failures that a fresh `claude auth login` on this machine fixes.
const NEEDS_LOGIN: &[&str] = &[
    "Không có đăng nhập Claude",
    "Claude CLI chưa có đăng nhập",
    "Claude CLI trên máy chưa đăng nhập",
];

fn needs_machine_login(error: &anyhow::Error) -> bool {
    let message = error.to_string();
    NEEDS_LOGIN.iter().any(|prefix| message.starts_with(prefix))
}

/// Keeps the real reason a machine sign-in failed instead of one generic message.
fn import_error(error: anyhow::Error) -> ProviderError {
    let message = error.to_string();
    let login = [LOGIN_BUSY_MESSAGE, LOGIN_FAILED_MESSAGE, LOGIN_TIMEOUT_MESSAGE];
    if OWN_MESSAGES.iter().chain(login.iter()).any(|prefix| message.starts_with(prefix)) {
        eprintln!("Claude import: {error:#}");
        return ProviderError::new(ProviderErrorKind::BadRequest, message);
    }
    if error.is::<tokio::time::error::Elapsed>() {
        eprintln!("Claude import: {error:#}");
        return ProviderError::new(
            ProviderErrorKind::GatewayTimeout,
            "Claude CLI không phản hồi khi kiểm tra đăng nhập (quá 15 giây)",
        );
    }
    provider_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_messages_are_kept() {
        let error = import_error(anyhow::anyhow!("Claude CLI trên máy chưa đăng nhập; chạy `claude` rồi đăng nhập"));
        assert!(format!("{error:?}").contains("trên máy chưa đăng nhập"));
    }

    #[test]
    fn missing_machine_login_triggers_browser_login() {
        assert!(needs_machine_login(&anyhow::anyhow!("Claude CLI chưa có đăng nhập có thể dùng lại; hãy đăng nhập qua web")));
        assert!(needs_machine_login(&anyhow::anyhow!("Không có đăng nhập Claude dạng file tương thích")));
        assert!(!needs_machine_login(&anyhow::anyhow!("os error 5")));
        let error = import_error(anyhow::anyhow!(LOGIN_TIMEOUT_MESSAGE));
        assert!(format!("{error:?}").contains("5 phút"));
    }

    #[test]
    fn other_errors_stay_generic() {
        let error = import_error(anyhow::anyhow!("os error 5"));
        assert!(!format!("{error:?}").contains("os error"));
    }
}
