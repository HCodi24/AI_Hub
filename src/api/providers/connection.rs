//! "Test connection" for every source kind, with a 60 second cache.
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::timeout;

#[derive(Clone, Serialize)]
pub(crate) struct ConnectionTest {
    ok: bool,
    pub(crate) status: &'static str,
    message: String,
    detail: String,
}

pub(super) async fn test_provider_uncached(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nhà cung cấp"))?;
    let result = match provider.kind.as_str() {
        "openai_compat" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "openai_compat" => {
            let url = format!("{}/models", provider.base_url.trim().trim_end_matches('/'));
            match timeout(
                Duration::from_secs(15),
                reqwest::Client::new()
                    .get(url)
                    .bearer_auth(&provider.api_key)
                    .send(),
            )
            .await
            {
                Err(_) => connection(
                    false,
                    "error",
                    "Không kết nối được",
                    "Hết thời gian chờ kết nối",
                ),
                Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
                Ok(Ok(response)) if response.status().is_success() => {
                    let detail = timeout(Duration::from_secs(15), response.json::<Value>())
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
                Ok(Ok(response)) => match response.status().as_u16() {
                    401 | 403 => connection(
                        false,
                        "error",
                        "Sai API key",
                        &format!("HTTP {}", response.status()),
                    ),
                    404 => connection(false, "error", "Sai Base URL", "HTTP 404"),
                    _ => connection(
                        false,
                        "error",
                        "Không kết nối được",
                        &format!("HTTP {}", response.status()),
                    ),
                },
            }
        }
        "anthropic" if provider.api_key.trim().is_empty() => connection(
            false,
            "not_configured",
            "Chưa nhập API key",
            "Chưa nhập API key",
        ),
        "anthropic" => match timeout(
            Duration::from_secs(15),
            reqwest::Client::new()
                .get("https://api.anthropic.com/v1/models")
                .header("x-api-key", &provider.api_key)
                .header("anthropic-version", "2023-06-01")
                .send(),
        )
        .await
        {
            Err(_) => connection(
                false,
                "error",
                "Không kết nối được",
                "Hết thời gian chờ kết nối",
            ),
            Ok(Err(e)) => connection(false, "error", "Không kết nối được", &e.to_string()),
            Ok(Ok(r)) if r.status().is_success() => connection(
                true,
                "connected",
                "Đã kết nối",
                "Anthropic API phản hồi thành công",
            ),
            Ok(Ok(r)) if r.status().as_u16() == 401 || r.status().as_u16() == 403 => connection(
                false,
                "error",
                "Sai API key",
                &format!("HTTP {}", r.status()),
            ),
            Ok(Ok(r)) => connection(
                false,
                "error",
                "Không kết nối được",
                &format!("HTTP {}", r.status()),
            ),
        },
        "codex_cli" => super::codex::test_connection(&state, &id).await,
        "claude_cli" => super::claude::test_connection(&state, &id).await,
        _ => connection(
            false,
            "error",
            "Không kết nối được",
            "Loại nhà cung cấp không được hỗ trợ",
        ),
    };
    Ok(Json(result))
}

pub(crate) async fn cached_provider_test(state: &AppState, id: &str) -> ApiResult<ConnectionTest> {
    if let Some((checked, status)) = state.provider_status.lock().await.get(id).cloned() {
        if checked.elapsed() < Duration::from_secs(60) {
            return Ok(status);
        }
    }
    let status = test_provider_uncached(State(state.clone()), Path(id.to_owned()))
        .await?
        .0;
    state
        .provider_status
        .lock()
        .await
        .insert(id.to_owned(), (Instant::now(), status.clone()));
    Ok(status)
}

pub(crate) async fn test_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<ConnectionTest>> {
    if let Some(session) = state.claude_login.lock().await.as_mut() {
        if session.provider_id == id {
            session.last_activity = Instant::now();
        }
    }
    state.provider_status.lock().await.remove(&id);
    cached_provider_test(&state, &id).await.map(Json)
}

pub(super) fn connection(ok: bool, status: &'static str, message: &str, detail: &str) -> ConnectionTest {
    ConnectionTest {
        ok,
        status,
        message: message.into(),
        detail: detail.into(),
    }
}
