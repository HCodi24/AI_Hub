//! MutiS entry point and early-stop detection for the Codex app-server adapter.
use super::{codex::CodexProvider, Provider, ProviderEvent, ProviderRequest};
use anyhow::{ensure, Result};
use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// The skill directory under the app-server's per-source CODEX_HOME, without creating it.
pub(crate) fn managed_skills_dir(config_path: &Path, provider_id: &str) -> Result<PathBuf> {
    ensure!(
        provider_id.len() <= 80
            && crate::features::addons::safe_component(provider_id.as_ref())
            && provider_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte)),
        "Mã nguồn Codex không hợp lệ"
    );
    Ok(crate::core::paths::data_dir(config_path)?
        .join("codex")
        .join(provider_id)
        .join("skills"))
}

pub(crate) struct CodexCliProvider;

#[async_trait]
impl Provider for CodexCliProvider {
    async fn run(
        &self,
        mut request: ProviderRequest,
        events: mpsc::Sender<ProviderEvent>,
        cancel: CancellationToken,
    ) -> Result<()> {
        // Native skill instructions are disabled because the native catalogue also lists project skills.
        let config_path = crate::config::config_file_path()?;
        if let Ok(dir) = managed_skills_dir(&config_path, &request.provider.id) {
            let catalogue = crate::features::addons::managed_skill_context(&dir);
            if !catalogue.is_empty() {
                if let Some(message) = request.messages.last_mut() {
                    message.content =
                        format!("{catalogue}\nYêu cầu của người dùng\n{}", message.content);
                }
            }
        }
        CodexProvider.run(request, events, cancel).await
    }
}

pub const EARLY_STOP_PHRASES: &[&str] = &[
    "incomplete",
    "not finished",
    "chưa xong",
    "chưa hoàn thành",
    "remaining work",
    "i stopped",
];

pub fn indicates_early_stop(message: &str) -> bool {
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let lowered = normalized
        .trim()
        .split("\n\n")
        .last()
        .unwrap_or_default()
        .to_lowercase();
    EARLY_STOP_PHRASES
        .iter()
        .any(|phrase| lowered.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_stop_checks_only_the_last_paragraph() {
        assert!(indicates_early_stop(
            "Summary\n\nRemaining work: finish tests"
        ));
        assert!(!indicates_early_stop("Previously incomplete\n\nAll done."));
        assert!(indicates_early_stop("CHƯA HOÀN THÀNH"));
    }
}
