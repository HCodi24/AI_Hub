use crate::{
    config::ProviderConfig,
    features::providers::error::{ProviderError, ProviderErrorKind, ProviderResult},
    providers::{
        antigravity_cli::{clean_home, SIGNED_OUT_FILE},
        cli_support,
    },
};
use std::{process::Stdio, time::Duration};
use tokio::time::timeout;

/// Model ids in `agy models` stdout are in the first tab-separated column.
/// Skip headers, blanks and duplicates.
pub(crate) fn parse_models(stdout: &str) -> Vec<String> {
    let mut models: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let id = line.split('\t').next().unwrap_or("").trim();
        let header = id.eq_ignore_ascii_case("id") || id.eq_ignore_ascii_case("model");
        if !id.is_empty()
            && !header
            && !id.contains(char::is_whitespace)
            && !models.iter().any(|known| known == id)
        {
            models.push(id.to_owned());
        }
    }
    models
}

/// Model ids from `agy models` for the provider's own sign-in home.
pub(crate) async fn list_models(provider: &ProviderConfig) -> ProviderResult<Vec<String>> {
    let bad_gateway = |message: String| ProviderError::new(ProviderErrorKind::BadGateway, message);
    let home = clean_home(&provider.id).map_err(|err| bad_gateway(err.to_string()))?;
    if home.join(SIGNED_OUT_FILE).try_exists().unwrap_or(false) {
        return Err(bad_gateway("Antigravity CLI chưa đăng nhập trong MutiS".into()));
    }
    let path = cli_support::resolve_executable("agy", Some(&provider.executable))
        .ok_or_else(|| {
            ProviderError::new(
                ProviderErrorKind::NotFound,
                "Chưa tìm thấy Antigravity CLI (agy) trên máy",
            )
        })?;
    let mut command = cli_support::command_for(&path, &["models".into()]);
    cli_support::agy_environment(&mut command, &home);
    command.stdin(Stdio::null()).kill_on_drop(true);
    let output = timeout(Duration::from_secs(25), command.output())
        .await
        .map_err(|_| bad_gateway("Hết thời gian chờ `agy models`".into()))?
        .map_err(|err| bad_gateway(format!("Không chạy được agy: {err}")))?;
    let models = parse_models(&String::from_utf8_lossy(&output.stdout));
    if !output.status.success() || models.is_empty() {
        return Err(bad_gateway(
            "Antigravity không trả model nào; hãy đăng nhập Antigravity rồi thử lại".into(),
        ));
    }
    Ok(models)
}
