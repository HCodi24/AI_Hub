use crate::{
    chat::Permission,
    providers::{ProviderEvent, codex::{gate::{self, Gate, Verdict}, transport::Client}},
};
use anyhow::Result;
use serde_json::{Value, json};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) async fn permission_denied(events: &mpsc::Sender<ProviderEvent>) {
    let _ = events
        .send(ProviderEvent {
            name: "permission_denied",
            data: json!({"need": "write"}),
        })
        .await;
}

pub(crate) async fn handle(
    client: &mut Client,
    gate: &Gate,
    permission: Permission,
    value: &Value,
    events: &mpsc::Sender<ProviderEvent>,
    cancel: &CancellationToken,
    denial_sent: &mut bool,
) -> Result<bool> {
    // Read must stay read-only unless the user says otherwise: a write the sandbox would have stopped is put to them.
    let method = value["method"].as_str().unwrap_or("");
    let file_edit = method == "item/fileChange/requestApproval";
    let command = value["params"]["command"].as_str().unwrap_or("");
    if permission == Permission::Read && (file_edit || (method == "item/commandExecution/requestApproval" && gate::writes(command))) {
        let shown = if file_edit { value["params"]["reason"].as_str().unwrap_or("Sửa file").to_owned() } else { crate::core::text::truncate_chars(command, 300) };
        let Some(allowed) = crate::providers::approvals::confirm(events, cancel, shown).await else { return Ok(false) };
        if allowed {
            // The command now runs outside the sandbox: a later failure is the command's own, never a permission block.
            *denial_sent = true;
            crate::providers::emit_text(events, "log", "Bạn đã cho phép lệnh ghi này một lần.".into()).await?;
            client.respond(value, gate::decision(&Verdict::Allow)).await?;
        } else {
            crate::providers::emit_text(events, "log", "Lệnh ghi bị từ chối (Chỉ đọc).".into()).await?;
            permission_denied(events).await;
            client.respond(value, gate::decision(&Verdict::Deny(Vec::new()))).await?;
        }
        return Ok(true);
    }
    match gate.judge(value["method"].as_str().unwrap_or(""), &value["params"]) {
        Some(verdict) => {
            if let Verdict::Deny(paths) = &verdict {
                crate::providers::emit_text(events, "log", format!("Đã chặn lệnh đọc ngoài thư mục: {}", crate::core::text::truncate_chars(&paths.join(", "), 220))).await?;
            }
            client.respond(value, gate::decision(&verdict)).await?;
        }
        None => client.reject_request(value).await?,
    }
    Ok(true)
}
