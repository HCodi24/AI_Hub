use crate::providers::codex::transport::Client;
use serde_json::json;
use std::time::Duration;

/// Windows needs a one-time sandbox setup per Codex home before Codex may write files; elsewhere it is always ready.
pub(crate) async fn sandbox_status(client: &mut Client) -> String {
    if !cfg!(windows) {
        return "ready".into();
    }
    match client.call("windowsSandbox/readiness", json!({})).await {
        Ok(value) => value["status"].as_str().unwrap_or("unknown").to_owned(),
        Err(_) => "unknown".into(),
    }
}

/// Runs the elevated sandbox setup. Windows shows its administrator prompt; the user has a few minutes to accept.
pub(super) async fn setup_sandbox(client: &mut Client) -> anyhow::Result<()> {
    let started = client.call("windowsSandbox/setupStart", json!({"mode":"elevated"})).await?;
    anyhow::ensure!(started["started"].as_bool().unwrap_or(false), "Codex không bắt đầu thiết lập sandbox");
    tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            let message = client.next().await?;
            if message["method"] == "windowsSandbox/setupCompleted" {
                if message["params"]["success"].as_bool().unwrap_or(false) {
                    return Ok(());
                }
                anyhow::bail!("Thiết lập sandbox thất bại: {}", message["params"]["error"].as_str().unwrap_or("không rõ lý do"));
            }
        }
    })
    .await
    .map_err(|_| anyhow::anyhow!("Hết 4 phút chờ thiết lập sandbox (đã bấm Có ở hộp thoại quyền quản trị chưa?)"))?
}
