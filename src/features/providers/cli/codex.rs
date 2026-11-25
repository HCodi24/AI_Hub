use crate::{
    app::AppState,
    config::{Config, ProviderConfig},
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
    providers::{
        cli_support,
        codex::{home, transport::Client},
    },
};
use std::time::Instant;

pub(crate) struct CodexConnection {
    pub(crate) client: Client,
    pub(crate) login_id: Option<String>,
    pub(crate) url: Option<String>,
    pub(crate) started: Instant,
}

impl CodexConnection {
    pub(crate) fn pending(client: Client, login_id: Option<String>, url: String) -> Self {
        Self {
            client,
            login_id,
            url: Some(url),
            started: std::time::Instant::now(),
        }
    }
}

pub(crate) fn provider_error(error: anyhow::Error) -> ProviderError {
    ProviderError::new(ProviderErrorKind::BadGateway, error.to_string())
}

pub(crate) async fn new_client(state: &AppState, provider: &ProviderConfig) -> ProviderResult<Client> {
    let executable = cli_support::resolve_executable("codex", Some(&provider.executable))
        .ok_or_else(|| {
            ProviderError::new(ProviderErrorKind::NotFound, "Chưa tìm thấy Codex CLI trên máy")
        })?;
    let dir = home(&state.config_path, provider).map_err(provider_error)?;
    Client::start_without_plugins(&executable, &dir, &dir).await.map_err(provider_error)
}

pub(crate) async fn update_config(
    state: &AppState,
    message: &'static str,
    update: impl FnOnce(&mut Config),
) -> ProviderResult<()> {
    let mut config = state.config.write().await;
    update(&mut config);
    config.codex.revision = config.codex.revision.wrapping_add(1);
    crate::config::save(&state.config_path, &config)
        .map_err(|_| ProviderError::new(ProviderErrorKind::Internal, message))
}

/// Deletes only this source's credential file; the tokens stay valid wherever else they are used.
pub(crate) fn remove_local_auth(dir: &std::path::Path) -> anyhow::Result<()> {
    match std::fs::remove_file(dir.join("auth.json")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::anyhow!("Không xoá được đăng nhập Codex của nguồn này: {e}")),
    }
}

pub(crate) fn allowed_url(value: &str) -> bool {
    reqwest::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && matches!(
                url.host_str(),
                Some("auth.openai.com" | "auth0.openai.com" | "chatgpt.com")
            )
    })
}
