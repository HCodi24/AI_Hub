use crate::{
    app::AppState,
    config::ProviderConfig,
    features::providers::{
        cli::codex::{
            allowed_url, new_client, provider_error as error, remove_local_auth, update_config,
            CodexConnection,
        },
        error::{ProviderError, ProviderErrorKind, ProviderResult},
    },
    providers::{cli_support, codex::home},
};
use serde_json::json;
use std::collections::HashMap;

pub(crate) async fn start_web_login(
    state: &AppState,
    provider: &ProviderConfig,
    id: &str,
    connections: &mut HashMap<String, CodexConnection>,
) -> ProviderResult<()> {
    // Drop the previous account locally (never revoke: it may be shared with the machine's Codex),
    // then start Codex without credentials so it begins a fresh sign-in.
    let dir = home(&state.config_path, provider).map_err(error)?;
    remove_local_auth(&dir).map_err(error)?;
    // The previous account is gone; this marker only counts once the web sign-in connects.
    cli_support::set_auth_origin(&dir, Some("web"));
    let mut client = new_client(state, provider).await?;
    // Ensure subsequent conversations do not resume an earlier account's context.
    update_config(state, "Không lưu được cấu hình Codex", |_| {}).await?;
    let result = client
        .call("account/login/start", json!({"type":"chatgpt"}))
        .await
        .map_err(error)?;
    let url = result["authUrl"]
        .as_str()
        .filter(|url| allowed_url(url))
        .ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::BadGateway,
                "Codex trả địa chỉ đăng nhập không hợp lệ",
            )
        })?
        .to_owned();
    let login_id = result["loginId"].as_str().map(str::to_owned);
    connections.insert(
        id.to_owned(),
        CodexConnection::pending(client, login_id, url),
    );
    Ok(())
}
