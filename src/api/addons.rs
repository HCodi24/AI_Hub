//! HTTP API for "Plugin & Skill" (see features/addons.rs).

use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::addons,
    providers::cli_support,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{ffi::OsString, path::PathBuf, process::Stdio, time::Duration};

#[derive(Deserialize)]
pub(crate) struct Change {
    /// "claude" (AI Hub's Claude Code) or "codex".
    target: String,
    /// "plugin" | "skill" | "mcp".
    kind: String,
    name: String,
    /// "add" (copy from the user's Claude Code) or "remove".
    action: String,
}

fn bad(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, message)
}

fn hub_dir() -> ApiResult<PathBuf> {
    cli_support::ai_hub_claude_config_dir().ok_or_else(|| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không tạo được thư mục cấu hình Claude của MutiS",
        )
    })
}

pub(crate) async fn overview() -> ApiResult<Json<addons::Overview>> {
    let dir = hub_dir()?;
    tokio::task::spawn_blocking(move || addons::overview(&dir))
        .await
        .map(Json)
        .map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không đọc được danh sách plugin",
            )
        })
}

/// The CLI configured for the first source of `kind`, or one found on the machine.
async fn executable(state: &AppState, kind: &str, name: &str) -> ApiResult<PathBuf> {
    let explicit = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.kind == kind)
        .map(|provider| provider.executable.clone());
    cli_support::resolve_executable(name, explicit.as_deref()).ok_or_else(|| {
        bad(format!(
            "Không tìm thấy {name} CLI trên máy (xem Cài đặt › Nguồn AI)"
        ))
    })
}

/// Run one CLI command to the end and return its combined output (last 4000 chars). `claude` commands run against
/// AI Hub's own Claude config.
async fn run_cli(path: &std::path::Path, args: &[&str], claude: bool) -> ApiResult<String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut command = cli_support::command_for(path, &args);
    if claude {
        cli_support::clean_claude_environment(&mut command);
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(180), command.output())
        .await
        .map_err(|_| bad("Lệnh chạy quá 3 phút và đã bị dừng"))?
        .map_err(|error| bad(format!("Không chạy được lệnh: {error}")))?;
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    let text: String = {
        let chars: Vec<char> = text.trim().chars().collect();
        chars[chars.len().saturating_sub(4000)..].iter().collect()
    };
    if !output.status.success() {
        return Err(bad(if text.is_empty() {
            "Lệnh thất bại".to_owned()
        } else {
            text
        }));
    }
    Ok(text)
}

pub(crate) async fn change(
    State(state): State<AppState>,
    Json(input): Json<Change>,
) -> ApiResult<Json<Value>> {
    if !addons::valid_name(&input.name) {
        return Err(bad("Tên không hợp lệ"));
    }
    let add = match input.action.as_str() {
        "add" => true,
        "remove" => false,
        _ => return Err(bad("Hành động không hợp lệ")),
    };
    let name = input.name.as_str();
    let output = match (input.target.as_str(), input.kind.as_str()) {
        ("claude", "skill") => {
            let hub = hub_dir()?;
            let personal = addons::personal_claude_dir()
                .ok_or_else(|| bad("Không tìm được thư mục người dùng"))?;
            let name = name.to_owned();
            tokio::task::spawn_blocking(move || {
                if add {
                    addons::copy_skill(&personal, &hub, &name)
                } else {
                    addons::remove_skill(&hub, &name)
                }
            })
            .await
            .map_err(|_| bad("Không xử lý được skill"))?
            .map_err(|error| bad(error.to_string()))?;
            String::new()
        }
        ("claude", "plugin") => {
            let claude = executable(&state, "claude_cli", "claude").await?;
            if add {
                // The plugin's marketplace must be known to AI Hub's Claude first; it is the one the user installed from.
                let hub = hub_dir()?;
                let personal = tokio::task::spawn_blocking(move || addons::overview(&hub).personal)
                    .await
                    .map_err(|_| bad("Không đọc được Claude Code của bạn"))?;
                let source = personal
                    .plugins
                    .iter()
                    .find(|plugin| plugin.id == name)
                    .map(|plugin| plugin.marketplace_source.clone())
                    .ok_or_else(|| bad(format!("Claude Code của bạn không có plugin «{name}»")))?;
                if source.is_empty() {
                    return Err(bad("Không biết plugin này được cài từ marketplace nào"));
                }
                // Already-added marketplaces make this command fail; the install below reports any real problem.
                let _ = run_cli(&claude, &["plugin", "marketplace", "add", &source], true).await;
                run_cli(
                    &claude,
                    &["plugin", "install", name, "--scope", "user"],
                    true,
                )
                .await?
            } else {
                run_cli(
                    &claude,
                    &["plugin", "uninstall", name, "--scope", "user"],
                    true,
                )
                .await?
            }
        }
        ("claude", "mcp") => {
            let claude = executable(&state, "claude_cli", "claude").await?;
            if add {
                let name_owned = name.to_owned();
                let definition = tokio::task::spawn_blocking(move || {
                    addons::personal_mcp_definition(&name_owned)
                })
                .await
                .map_err(|_| bad("Không đọc được MCP server"))?
                .map_err(|error| bad(error.to_string()))?;
                let json = definition.to_string();
                run_cli(
                    &claude,
                    &["mcp", "add-json", name, &json, "--scope", "user"],
                    true,
                )
                .await?
            } else {
                run_cli(&claude, &["mcp", "remove", name, "--scope", "user"], true).await?
            }
        }
        ("codex", "mcp") => {
            let codex = executable(&state, "codex_cli", "codex").await?;
            if add {
                let name_owned = name.to_owned();
                let definition = tokio::task::spawn_blocking(move || {
                    addons::personal_mcp_definition(&name_owned)
                })
                .await
                .map_err(|_| bad("Không đọc được MCP server"))?
                .map_err(|error| bad(error.to_string()))?;
                let Some(command) = definition["command"].as_str() else {
                    return Err(bad("Chỉ chép được MCP server dạng lệnh (stdio) sang Codex"));
                };
                let mut args: Vec<String> = vec!["mcp".into(), "add".into(), name.into()];
                if let Some(env) = definition["env"].as_object() {
                    for (key, value) in env {
                        if let Some(value) = value.as_str() {
                            args.extend(["--env".into(), format!("{key}={value}")]);
                        }
                    }
                }
                args.extend(["--".into(), command.to_owned()]);
                if let Some(list) = definition["args"].as_array() {
                    args.extend(
                        list.iter()
                            .filter_map(|arg| arg.as_str().map(str::to_owned)),
                    );
                }
                let refs: Vec<&str> = args.iter().map(String::as_str).collect();
                run_cli(&codex, &refs, false).await?
            } else {
                run_cli(&codex, &["mcp", "remove", name], false).await?
            }
        }
        _ => return Err(bad("Loại này chưa được hỗ trợ")),
    };
    Ok(Json(json!({"ok": true, "output": output})))
}
