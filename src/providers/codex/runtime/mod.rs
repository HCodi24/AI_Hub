use super::{environment, transport::Client};
use crate::{
    features::providers::permission::codex_approval as approval,
    providers::{cli_support, Provider, ProviderEvent, ProviderRequest},
};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::json;
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod thread;
mod turn;

#[cfg(test)]
mod tests;

pub struct CodexProvider;

#[async_trait]
impl Provider for CodexProvider {
    async fn run(
        &self,
        mut request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        let config_path = crate::config::config_file_path()?;
        let settings = environment::settings()?;
        let home = environment::home(&config_path, &request.provider)?;
        let executable =
            cli_support::resolve_executable("codex", Some(&request.provider.executable))
                .context("Chưa tìm thấy Codex; vào Cài đặt → Nguồn AI để kết nối")?;
        let folder = Path::new(&request.folder);
        let prefix = environment::session_prefix(&settings, &request.provider, &request.folder);
        let native_id = request
            .session_id
            .as_deref()
            .and_then(|id| id.strip_prefix(&prefix))
            .map(str::to_owned);
        let mut client = Client::start(&executable, &home, folder).await?;
        let result = async {
            let (id, resumed, gate) = thread::open(
                &mut client,
                &mut request,
                &config_path,
                &settings,
                &home,
                native_id,
                &events,
                &cancel,
            )
            .await?;
            events
                .send(ProviderEvent {
                    name: "session",
                    data: json!({"session_id": format!("{prefix}{id}")}),
                })
                .await?;
            turn::run(&mut client, &request, &gate, &id, resumed, &events, &cancel).await
        }
        .await;
        client.close().await;
        result
    }
}
