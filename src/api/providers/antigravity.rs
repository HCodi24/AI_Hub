//! Antigravity CLI uses the machine's sign-in. MutiS stores only its local sign-out state.
use super::connection::{connection, ConnectionTest};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    config::ProviderConfig,
    providers::{antigravity_cli::{clean_home, SIGNED_OUT_FILE}, cli_support},
};
use axum::{extract::{Path, State}, http::StatusCode, Json};
use serde::Deserialize;
use std::{io, path::Path as FsPath, process::Stdio, time::Duration};
use tokio::time::timeout;

/// The signed-in account's email, saved by the web sign-in (agy prints "Logged in as <email>." and has no command that shows it later).
pub(crate) const EMAIL_FILE: &str = ".mutis-email";

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Action {
    Cli,
    Logout,
}

pub(crate) async fn antigravity_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(action): Json<Action>,
) -> ApiResult<Json<ConnectionTest>> {
    let provider = state.config.read().await.providers.iter()
        .find(|provider| provider.id == id && provider.kind == "antigravity_cli")
        .cloned()
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy nguồn Antigravity CLI"))?;
    let error = |error: &dyn std::fmt::Display| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string());
    let home = clean_home(&id).map_err(|err| error(&err))?;
    let marker = home.join(SIGNED_OUT_FILE);
    let result = match action {
        Action::Cli => {
            let result = probe(&provider, &home).await;
            if result.status == "connected" {
                match std::fs::remove_file(&marker) {
                    Ok(()) => {},
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {},
                    Err(err) => return Err(error(&err)),
                }
            }
            result
        }
        // A web account owns its sign-in (a token file in its own home): signing out deletes it. A machine source only remembers the sign-out.
        Action::Logout if provider.web_login => {
            for file in [home.join(".gemini").join("antigravity-cli").join("antigravity-oauth-token"), home.join(EMAIL_FILE)] {
                match std::fs::remove_file(&file) {
                    Ok(()) => {}
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                    Err(err) => return Err(error(&err)),
                }
            }
            signed_out()
        }
        Action::Logout => {
            std::fs::write(&marker, []).map_err(|err| error(&err))?;
            signed_out()
        }
    };
    state.provider_status.lock().await.remove(&id);
    Ok(Json(result))
}

fn signed_out() -> ConnectionTest {
    connection(false, "not_configured", "Antigravity CLI chưa đăng nhập trong MutiS", "")
}

/// Counts the models in `agy models` stdout (one `slug<TAB>name` line each).
fn model_count(stdout: &str) -> usize {
    stdout.lines().filter(|line| !line.trim().is_empty()).count()
}

/// Model ids in `agy models` stdout: the first tab-separated column of each row, without headers, blanks and duplicates.
fn parse_models(stdout: &str) -> Vec<String> {
    let mut models: Vec<String> = Vec::new();
    for line in stdout.lines() {
        let id = line.split('\t').next().unwrap_or("").trim();
        let header = id.eq_ignore_ascii_case("id") || id.eq_ignore_ascii_case("model");
        if !id.is_empty() && !header && !id.contains(char::is_whitespace) && !models.iter().any(|known| known == id) {
            models.push(id.to_owned());
        }
    }
    models
}

/// Model ids from `agy models` for the provider's own sign-in home.
pub(super) async fn list_models(provider: &ProviderConfig) -> ApiResult<Vec<String>> {
    let bad_gateway = |message: String| ApiError::new(StatusCode::BAD_GATEWAY, message);
    let home = clean_home(&provider.id).map_err(|err| bad_gateway(err.to_string()))?;
    if home.join(SIGNED_OUT_FILE).try_exists().unwrap_or(false) {
        return Err(bad_gateway("Antigravity CLI chưa đăng nhập trong MutiS".into()));
    }
    let path = cli_support::resolve_executable("agy", Some(&provider.executable))
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Chưa tìm thấy Antigravity CLI (agy) trên máy"))?;
    let mut command = cli_support::command_for(&path, &["models".into()]);
    cli_support::agy_environment(&mut command, &home);
    command.stdin(Stdio::null()).kill_on_drop(true);
    let output = timeout(Duration::from_secs(25), command.output())
        .await
        .map_err(|_| bad_gateway("Hết thời gian chờ `agy models`".into()))?
        .map_err(|err| bad_gateway(format!("Không chạy được agy: {err}")))?;
    let models = parse_models(&String::from_utf8_lossy(&output.stdout));
    if !output.status.success() || models.is_empty() {
        return Err(bad_gateway("Antigravity không trả model nào; hãy đăng nhập Antigravity rồi thử lại".into()));
    }
    Ok(models)
}

/// The plan (`Google AI Ultra`, ...) from `agy acp`: its `session/new` answer carries `_meta.antigravity.tier`. No email is available:
/// no agy command or ACP call prints one. Best effort; `None` when agy does not answer in time.
async fn plan(path: &FsPath, home: &FsPath) -> Option<String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let mut command = cli_support::command_for(path, &["acp".into()]);
    cli_support::agy_environment(&mut command, home);
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).kill_on_drop(true);
    let mut child = command.spawn().ok()?;
    let mut stdin = child.stdin.take()?;
    let mut lines = BufReader::new(child.stdout.take()?).lines();
    let folder = std::env::temp_dir();
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": 1, "clientCapabilities": {}}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "session/new", "params": {"cwd": folder, "mcpServers": []}}),
    ];
    for request in requests {
        stdin.write_all(format!("{request}\n").as_bytes()).await.ok()?;
    }
    let read = async {
        while let Ok(Some(line)) = lines.next_line().await {
            let value: serde_json::Value = serde_json::from_str(&line).ok()?;
            if value["id"] == 2 {
                return value["result"]["_meta"]["antigravity"]["tier"]["name"].as_str().map(str::to_owned);
            }
        }
        None
    };
    timeout(Duration::from_secs(20), read).await.ok().flatten()
}

// shortcut: "signed in" is inferred from `agy models` succeeding with a list; agy has no whoami/status command, so no email is shown.
pub(super) async fn test_connection(provider: &ProviderConfig) -> ConnectionTest {
    let home = match clean_home(&provider.id) {
        Ok(home) => home,
        Err(err) => return connection(false, "error", "Không mở được thư mục Antigravity CLI", &err.to_string()),
    };
    match home.join(SIGNED_OUT_FILE).try_exists() {
        Ok(false) => probe(provider, &home).await,
        Ok(true) => signed_out(),
        Err(err) => connection(false, "error", "Không đọc được trạng thái Antigravity CLI", &err.to_string()),
    }
}

async fn probe(provider: &ProviderConfig, home: &FsPath) -> ConnectionTest {
    let Some(path) = cli_support::resolve_executable("agy", Some(&provider.executable)) else {
        return connection(false, "error", "Chưa tìm thấy Antigravity CLI (agy) trên máy", "irm https://antigravity.google/cli/install.ps1 | iex");
    };
    let run = |args: &[&str]| {
        let mut command = cli_support::command_for(&path, &args.iter().map(Into::into).collect::<Vec<_>>());
        cli_support::agy_environment(&mut command, home);
        command.stdin(Stdio::null()).kill_on_drop(true);
        async move { timeout(Duration::from_secs(25), command.output()).await }
    };
    match run(&["models"]).await {
        Err(_) => connection(false, "error", "Antigravity không phản hồi", "Hết thời gian chờ `agy models`"),
        Ok(Err(e)) => connection(false, "error", "Không chạy được agy", &e.to_string()),
        Ok(Ok(output)) => match model_count(&String::from_utf8_lossy(&output.stdout)) {
            count if output.status.success() && count > 0 => {
                // `/usage` prints the plan's remaining limits (one tab-separated line each); best effort, shown as the detail.
                let usage = match run(&["--print=/usage", "--output-format", "text"]).await {
                    Ok(Ok(out)) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_owned(),
                    _ => String::new(),
                };
                let mut result = connection(true, "connected", &format!("Đã đăng nhập Antigravity · {count} model"), &usage);
                result.plan = plan(&path, home).await;
                result.email = std::fs::read_to_string(home.join(EMAIL_FILE)).ok().map(|text| text.trim().to_owned()).filter(|text| !text.is_empty());
                result
            }
            _ => connection(false, "not_configured", "Antigravity chưa đăng nhập", "Chạy `agy` trong terminal và đăng nhập"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::model_count;

    #[test]
    fn counts_only_listed_models() {
        assert_eq!(model_count("gemini-4-neon-high\tGemini 4 Neon (High)\n\ngemini-4-carbon-low\tGemini 4 Carbon (Low)\n"), 2);
        assert_eq!(model_count("  \n"), 0);
    }
}
