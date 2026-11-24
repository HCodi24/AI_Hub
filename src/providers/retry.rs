use super::{emit_text, ProviderEvent};
use crate::config::MutisConfig;
use anyhow::{bail, Result};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub fn retryable_status(status: reqwest::StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
}

pub fn retry_delay(base_ms: u64, retry_index: u32, jitter_ms: u64) -> std::time::Duration {
    std::time::Duration::from_millis(
        base_ms
            .saturating_mul(1u64.checked_shl(retry_index).unwrap_or(u64::MAX))
            .saturating_add(jitter_ms.min(250)),
    )
}

/// `a: b: c` from an error and its sources (reqwest's own text stops at "error sending request").
fn error_chain(error: &dyn std::error::Error) -> String {
    let (mut text, mut source) = (error.to_string(), error.source());
    while let Some(cause) = source {
        text.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    text
}

pub async fn send_with_retry(
    client: &reqwest::Client,
    url: &str,
    headers: Vec<(String, String)>,
    body: &Value,
    config: &MutisConfig,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> Result<reqwest::Response> {
    let max_retries = if config.effective(config.retry_api) {
        config.retry_attempts
    } else {
        0
    };
    let mut retry = 0;
    loop {
        let mut builder = client.post(url).json(body);
        for (name, value) in &headers {
            builder = builder.header(name, value);
        }
        let result = tokio::select! { _ = cancel.cancelled() => bail!("cancelled"), result = builder.send() => result };
        let retryable = match &result {
            // A connection the server drops mid-request is `is_request`, not `is_connect`: it is just as transient.
            Err(error) => error.is_connect() || error.is_timeout() || error.is_request(),
            Ok(response) => super::retryable_status(response.status()),
        };
        if !retryable || retry >= max_retries {
            // The cause (timeout, connection reset, TLS...) must reach the user: the context alone hides it.
            return result.map_err(|error| anyhow::anyhow!("Không thể gửi yêu cầu API: {}", error_chain(&error)));
        }
        let reason = match &result {
            Err(error) => error_chain(error),
            Ok(response) => format!("HTTP {}", response.status()),
        };
        retry += 1;
        let detail = format!("Thử lại lần {retry}/{max_retries} sau lỗi {reason}");
        emit_text(events, "log", detail.clone()).await?;
        emit_text(events, "retry", detail).await?;
        let jitter = crate::core::time::now_secs()
            .wrapping_mul(31)
            .wrapping_add(retry as u64 * 73)
            % 251;
        tokio::select! { _ = cancel.cancelled() => bail!("cancelled"), _ = tokio::time::sleep(super::retry_delay(config.retry_base_ms, retry - 1, jitter)) => {} }
    }
}
