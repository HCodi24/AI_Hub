use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::{
        cli::codex::{provider_error as error, remove_local_auth, update_config},
        error::ProviderResult,
    },
    providers::{cli_support, codex::home},
};

pub(crate) async fn logout(state: &AppState, provider: &ProviderConfig) -> ProviderResult<()> {
    // Local sign-out only: `account/logout` revokes the tokens server-side, and a source copied
    // from this machine shares them with the user's own Codex/ChatGPT, which would be signed out too.
    let dir = home(&state.config_path, provider).map_err(error)?;
    remove_local_auth(&dir).map_err(error)?;
    cli_support::set_auth_origin(&dir, None);
    update_config(state, "Không lưu được cấu hình Codex", |_| {}).await
}
