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
            anyhow::bail!("CLI chưa đăng nhập");
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

pub(crate) fn provider_error(error: anyhow::Error) -> crate::features::providers::error::ProviderError {
    use crate::features::providers::error::{ProviderError, ProviderErrorKind};
    eprintln!("Claude error: {error:#}");
    ProviderError::new(
        ProviderErrorKind::BadGateway,
        "Không xử lý được cấu hình hoặc đăng nhập Claude; kiểm tra bản cài và thử đăng nhập qua web",
    )
}
