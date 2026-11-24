use crate::{
    chat::Permission,
    config::CodexConfig,
    providers::{ProviderEvent, ProviderRequest, codex::{compact, environment, gate::Gate, transport::Client}},
};
use anyhow::{bail, Context, Result};
use serde_json::json;
use std::path::Path;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(super) async fn open(
    client: &mut Client,
    request: &mut ProviderRequest,
    config_path: &Path,
    settings: &CodexConfig,
    home: &Path,
    native_id: Option<String>,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
) -> Result<(String, bool, Gate)> {
    let folder = Path::new(&request.folder);
    let account = client.call("account/read", json!({"refreshToken":false})).await?;
    if account["account"].is_null() { bail!("Codex của MutiS chưa đăng nhập; dùng Tìm Codex CLI hoặc Đăng nhập qua web trong Cài đặt"); }
    let sandbox = if request.permission == Permission::Read { "read-only" } else { "workspace-write" };
    if sandbox == "workspace-write" && crate::api::providers::codex_sandbox_status(client).await != "ready" {
        bail!("Codex riêng của MutiS chưa thiết lập sandbox Windows nên không ghi được file. Vào Cài đặt › Nguồn AI › Codex bấm «Thiết lập sandbox» (cần xác nhận quyền quản trị), hoặc chọn quyền Chỉ đọc.");
    }
    let context = environment::project_context(folder, settings)? + &environment::plugin_index(home);
    let mcp = request.mcp.iter().filter(|server| server.enabled).map(|server| (
        server.name.clone(), json!({"command":server.command,"args":server.args,"env":server.env})
    )).collect::<serde_json::Map<String, serde_json::Value>>();
    // `untrusted`: Codex asks the app before running a command, and the gate declines paths outside the folder.
    let mut gate = Gate::new(folder, request.messages.iter().filter(|m| m.role == "user").map(|m| m.content.as_str()));
    gate.allow(&home.join("plugins/cache"));
    let params = json!({"cwd":request.folder,"model":request.model,"approvalPolicy":"untrusted","sandbox":sandbox,
        "developerInstructions":context,"selectedCapabilityRoots":[],"config":{"mcp_servers":mcp}});
    let mut resumed = native_id.is_some();
    let thread = if let Some(id) = native_id {
        let mut resume = params.clone();
        resume["threadId"] = json!(id);
        match client.call("thread/resume", resume).await {
            Ok(value) => value,
            Err(_) => {
                resumed = false;
                // Rebuild app context if the persisted native thread is no longer available.
                if request.messages.last().is_some_and(|m| m.content.contains(crate::features::memory::INSTRUCTION)) {
                    let config: crate::config::Config = serde_json::from_slice(&std::fs::read(config_path)?)?;
                    request.session_id = None;
                    let memory = crate::features::memory::memory_context(config_path, &request.folder, &config.memory);
                    if !memory.is_empty() {
                        if let Some(message) = request.messages.last_mut() { message.content = format!("{memory}\n\n{}", message.content); }
                    }
                }
                crate::providers::emit_text(events, "log", "Đang khôi phục phiên Codex từ ngữ cảnh MutiS".into()).await?;
                client.call("thread/start", params).await?
            }
        }
    } else { client.call("thread/start", params).await? };
    let id = thread["thread"]["id"].as_str().context("Codex không trả mã phiên")?.to_owned();
    // A thread that has grown past the budget is summarised in place before its next turn instead of being dropped.
    if resumed {
        if let Some(size) = compact::needs_compaction(&id, compact::thread_token_budget(config_path)) {
            crate::providers::emit_text(events, "log", format!("Luồng Codex đã đọc {size} token; đang nén ngữ cảnh.")).await?;
            match compact::compact(client, &id, cancel).await {
                Ok(()) => crate::providers::emit_text(events, "log", "Đã nén ngữ cảnh luồng Codex.".into()).await?,
                Err(error) => crate::providers::emit_text(events, "log", format!("Không nén được luồng Codex, dùng tiếp như cũ: {error}")).await?,
            }
        }
    }
    Ok((id, resumed, gate))
}
