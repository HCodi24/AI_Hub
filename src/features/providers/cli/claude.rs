use crate::providers::{claude::environment, cli_support};
use std::{path::Path, process::Stdio, time::Duration};

pub(crate) async fn auth_status(
    executable: &Path,
    home: &Path,
) -> anyhow::Result<cli_support::AuthAccount> {
    // Never mistake a machine-wide/keychain login for this source's file credentials.
    if !home.join(".credentials.json").is_file() {
        return Ok(cli_support::AuthAccount::default());
    }
    let mut command = cli_support::command_for(executable, &["auth".into(), "status".into()]);
    environment::apply(&mut command, home);
    let output = tokio::time::timeout(
        Duration::from_secs(15),
        command
            .current_dir(home)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .output(),
    )
    .await??;
    cli_support::parse_auth_account(&output.stdout)
        .ok_or_else(|| anyhow::anyhow!("Claude auth status không trả JSON hợp lệ"))
}

pub(crate) async fn import_cli_account(executable: &Path, home: &Path) -> anyhow::Result<()> {
    // Validate the allow-listed credentials in a temporary app-owned directory first.
    let stage = home.join(format!(".auth-check-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&stage)?;
    let result = async {
        environment::import_cli_auth(&stage)?;
        if !auth_status(executable, &stage).await?.logged_in {
            anyhow::bail!("Claude CLI trên máy chưa đăng nhập hoặc phiên đã hết hạn; bấm Đăng nhập để thử lại, hoặc dùng tab Web");
        }
        environment::import_auth_file(&stage.join(".credentials.json"), home)?;
        cli_support::set_auth_origin(home, Some("cli"));
        Ok(())
    }
    .await;
    // Only remove files created by this operation, never recursively delete a computed path.
    let _ = std::fs::remove_file(stage.join(".credentials.json"));
    let _ = std::fs::remove_file(stage.join(".claude.json"));
    let _ = std::fs::remove_dir(&stage);
    result
}

/// Longest wait for the user to finish signing in on the browser page `claude auth login` opens.
const MACHINE_LOGIN_WAIT: Duration = Duration::from_secs(300);
pub(crate) const LOGIN_TIMEOUT_MESSAGE: &str =
    "Hết thời gian chờ đăng nhập Claude trên trình duyệt (5 phút); bấm Đăng nhập để thử lại";
pub(crate) const LOGIN_FAILED_MESSAGE: &str =
    "Đăng nhập Claude trên trình duyệt chưa hoàn tất; bấm Đăng nhập để thử lại hoặc dùng tab Web";
pub(crate) const LOGIN_BUSY_MESSAGE: &str =
    "Đang chờ đăng nhập Claude trên trình duyệt; hoàn tất ở trang vừa mở";
static MACHINE_LOGIN_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Runs `claude auth login` for this machine's own Claude CLI (the one `import_cli_account` copies from).
/// It opens the browser and returns once the user has signed in, so the import can then be retried.
pub(crate) async fn login_machine_account(executable: &Path) -> anyhow::Result<()> {
    use std::sync::atomic::Ordering;
    if MACHINE_LOGIN_RUNNING.swap(true, Ordering::SeqCst) {
        anyhow::bail!(LOGIN_BUSY_MESSAGE);
    }
    struct Running;
    impl Drop for Running {
        fn drop(&mut self) {
            MACHINE_LOGIN_RUNNING.store(false, Ordering::SeqCst);
        }
    }
    let _running = Running;
    let mut command = cli_support::command_for(executable, &["auth".into(), "login".into()]);
    // Drop Claude variables inherited from a parent Claude session, but keep the machine config dir the import reads.
    let names = std::env::vars().map(|(name, _)| name).collect::<Vec<_>>();
    for name in cli_support::environment_names_to_remove(&names) {
        if name != "CLAUDE_CONFIG_DIR" {
            command.env_remove(name);
        }
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    match tokio::time::timeout(MACHINE_LOGIN_WAIT, child.wait()).await {
        Ok(status) if status.as_ref().is_ok_and(|status| status.success()) => Ok(()),
        Ok(status) => {
            eprintln!("Claude auth login: {status:?}");
            anyhow::bail!(LOGIN_FAILED_MESSAGE)
        }
        Err(_) => {
            cli_support::terminate_child_tree(&mut child).await;
            anyhow::bail!(LOGIN_TIMEOUT_MESSAGE)
        }
    }
}

pub(crate) fn provider_error(error: anyhow::Error) -> crate::features::providers::error::ProviderError {
    use crate::features::providers::error::{ProviderError, ProviderErrorKind};
    eprintln!("Claude error: {error:#}");
    ProviderError::new(
        ProviderErrorKind::BadGateway,
        "Không xử lý được cấu hình hoặc đăng nhập Claude; kiểm tra bản cài và thử đăng nhập qua web",
    )
}
