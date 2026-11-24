use crate::{
    config::ProviderConfig,
    features::providers::{
        cli::claude::{import_cli_account, provider_error},
        error::{ProviderError, ProviderErrorKind, ProviderResult},
        login_mode::ensure_login_mode,
    },
    providers::cli_support,
};
use std::path::Path;

pub(crate) async fn connect_machine_account(
    provider: &ProviderConfig,
    home: &Path,
) -> ProviderResult<()> {
    ensure_login_mode(provider, false)?;
    let executable = cli_support::resolve_executable("claude", Some(&provider.executable))
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::NotFound, "Chưa tìm thấy Claude CLI trên máy")
        })?;
    import_cli_account(&executable, home)
        .await
        .map_err(provider_error)
}
