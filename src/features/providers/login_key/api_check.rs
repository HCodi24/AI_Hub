use crate::features::providers::connection::{connection, ConnectionTest};
use crate::config::ProviderConfig;
use reqwest::{RequestBuilder, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;
use tokio::time::timeout;

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);

pub(crate) async fn openai(provider: &ProviderConfig) -> ConnectionTest {
    if provider.api_key.trim().is_empty() {
        return missing_key();
    }
    let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
    let response = send(reqwest::Client::new().get(url).bearer_auth(&provider.api_key)).await;
    match response {
        Ok(response) if response.status().is_success() => {
            let detail = timeout(CHECK_TIMEOUT, response.json::<Value>())
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Value::Null);
            let count = detail["data"].as_array().map_or(0, Vec::len);
            connection(
                true,
                "connected",
                &format!("Đã kết nối · {count} model"),
                &format!("Đã nhận {count} model từ {}", provider.base_url),
            )
        }
        Ok(response) => failed_status(response.status(), true),
        Err(result) => result,
    }
}

pub(crate) async fn anthropic(provider: &ProviderConfig) -> ConnectionTest {
    if provider.api_key.trim().is_empty() {
        return missing_key();
    }
    let response = send(
        reqwest::Client::new()
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", &provider.api_key)
            .header("anthropic-version", "2023-06-01"),
    )
    .await;
    match response {
        Ok(response) if response.status().is_success() => connection(
            true,
            "connected",
            "Đã kết nối",
            "Anthropic API phản hồi thành công",
        ),
        Ok(response) => failed_status(response.status(), false),
        Err(result) => result,
    }
}

async fn send(request: RequestBuilder) -> Result<Response, ConnectionTest> {
    match timeout(CHECK_TIMEOUT, request.send()).await {
        Err(_) => Err(connection_error("Hết thời gian chờ kết nối")),
        Ok(Err(error)) => Err(connection_error(&error.to_string())),
        Ok(Ok(response)) => Ok(response),
    }
}

fn missing_key() -> ConnectionTest {
    const MESSAGE: &str = "Chưa nhập API key";
    connection(false, "not_configured", MESSAGE, MESSAGE)
}

pub(crate) fn failed_status(status: StatusCode, openai: bool) -> ConnectionTest {
    match status.as_u16() {
        401 | 403 => connection(false, "error", "Sai API key", &format!("HTTP {status}")),
        404 if openai => connection(false, "error", "Sai Base URL", "HTTP 404"),
        _ => connection_error(&format!("HTTP {status}")),
    }
}

pub(crate) fn connection_error(detail: &str) -> ConnectionTest {
    connection(false, "error", "Không kết nối được", detail)
}
