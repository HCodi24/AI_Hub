use super::super::connection::{connection, ConnectionTest};
use super::{models::parse_models, signed_out, EMAIL_FILE};
use crate::{
    config::ProviderConfig,
    providers::{
        antigravity_cli::{clean_home, SIGNED_OUT_FILE},
        cli_support,
    },
};
use std::{path::Path, process::Stdio, time::Duration};
use tokio::time::timeout;

/// The plan (`Google AI Ultra`, ...) from `agy acp` is in `_meta.antigravity.tier`
/// in the `session/new` answer. No agy command or ACP call prints the email.
/// Returns `None` when agy does not answer in time.
async fn plan(path: &Path, home: &Path) -> Option<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut command = cli_support::command_for(path, &["acp".into()]);
    cli_support::agy_environment(&mut command, home);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = command.spawn().ok()?;
    let mut stdin = child.stdin.take()?;
    let mut lines = BufReader::new(child.stdout.take()?).lines();
    let folder = std::env::temp_dir();
    let requests = [
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": 1, "clientCapabilities": {}},
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "session/new",
            "params": {"cwd": folder, "mcpServers": []},
        }),
    ];
    for request in requests {
        stdin
            .write_all(format!("{request}\n").as_bytes())
            .await
            .ok()?;
    }
    let read = async {
        while let Ok(Some(line)) = lines.next_line().await {
            let value: serde_json::Value = serde_json::from_str(&line).ok()?;
            if value["id"] == 2 {
                return value["result"]["_meta"]["antigravity"]["tier"]["name"]
                    .as_str()
                    .map(str::to_owned);
            }
        }
        None
    };
    timeout(Duration::from_secs(20), read).await.ok().flatten()
}

// shortcut: Infer sign-in from `agy models` succeeding with a list.
// agy has no whoami/status command to retrieve the email.
pub(in crate::api::providers) async fn test_connection(provider: &ProviderConfig) -> ConnectionTest {
    let home = match clean_home(&provider.id) {
        Ok(home) => home,
        Err(err) => {
            return connection(
                false,
                "error",
                "Không mở được thư mục Antigravity CLI",
                &err.to_string(),
            );
        }
    };
    match home.join(SIGNED_OUT_FILE).try_exists() {
        Ok(false) => probe(provider, &home).await,
        Ok(true) => signed_out(),
        Err(err) => connection(
            false,
            "error",
            "Không đọc được trạng thái Antigravity CLI",
            &err.to_string(),
        ),
    }
}

pub(super) async fn probe(provider: &ProviderConfig, home: &Path) -> ConnectionTest {
    let Some(path) = cli_support::resolve_executable("agy", Some(&provider.executable)) else {
        return connection(
            false,
            "error",
            "Chưa tìm thấy Antigravity CLI (agy) trên máy",
            "irm https://antigravity.google/cli/install.ps1 | iex",
        );
    };
    let run = |args: &[&str]| {
        let mut command =
            cli_support::command_for(&path, &args.iter().map(Into::into).collect::<Vec<_>>());
        cli_support::agy_environment(&mut command, home);
        command.stdin(Stdio::null()).kill_on_drop(true);
        async move { timeout(Duration::from_secs(25), command.output()).await }
    };
    match run(&["models"]).await {
        Err(_) => connection(
            false,
            "error",
            "Antigravity không phản hồi",
            "Hết thời gian chờ `agy models`",
        ),
        Ok(Err(e)) => connection(false, "error", "Không chạy được agy", &e.to_string()),
        Ok(Ok(output)) => match parse_models(&String::from_utf8_lossy(&output.stdout)).len() {
            count if output.status.success() && count > 0 => {
                // Show the tab-separated remaining limits from `/usage` when available.
                let usage = match run(&["--print=/usage", "--output-format", "text"]).await {
                    Ok(Ok(out)) if out.status.success() => {
                        String::from_utf8_lossy(&out.stdout).trim().to_owned()
                    }
                    _ => String::new(),
                };
                let mut result = connection(
                    true,
                    "connected",
                    &format!("Đã đăng nhập Antigravity · {count} model"),
                    &usage,
                );
                result.plan = plan(&path, home).await;
                result.email = std::fs::read_to_string(home.join(EMAIL_FILE))
                    .ok()
                    .map(|text| text.trim().to_owned())
                    .filter(|text| !text.is_empty());
                result
            }
            _ => connection(
                false,
                "not_configured",
                "Antigravity chưa đăng nhập",
                "Chạy `agy` trong terminal và đăng nhập",
            ),
        },
    }
}
