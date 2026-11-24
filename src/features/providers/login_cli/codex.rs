use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::{
        cli::codex::{provider_error as codex_error, update_config},
        error::{ProviderError, ProviderErrorKind, ProviderResult},
    },
    providers::{cli_support, codex::home},
};

pub(crate) async fn connect_machine_account(
    state: &AppState,
    provider: &mut ProviderConfig,
    id: &str,
) -> ProviderResult<()> {
    let executable = cli_support::resolve_executable("codex", Some(&provider.executable))
        .ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::NotFound,
                "Không tìm thấy Codex; hãy cài Codex CLI hoặc chọn đường dẫn chương trình",
            )
        })?;
    provider.executable = executable.to_string_lossy().into_owned();
    let dir = home(&state.config_path, provider).map_err(codex_error)?;
    crate::providers::codex::environment::import_cli_auth(&dir).map_err(codex_error)?;
    cli_support::set_auth_origin(&dir, Some("cli"));
    update_config(state, "Không lưu được kết nối Codex", |config| {
        if let Some(p) = config.providers.iter_mut().find(|p| p.id == id) {
            p.executable = provider.executable.clone();
        }
    })
    .await
}
