use super::{
    antigravity::{agy_cli, claude_plugin_paths},
    bad, claude_dir,
    cli::run_cli_in,
    codex::{codex_cli, codex_list},
    executable,
};
use crate::{
    api::{ApiError, ApiResult},
    app::AppState,
    features::addons,
};
use axum::{extract::State, http::StatusCode, Json};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::Path;

#[derive(Deserialize)]
pub(crate) struct Change {
    /// "claude" | "codex" | "mutis".
    target: String,
    /// "plugin" | "skill" | "mcp".
    kind: String,
    name: String,
    /// "add" (copy from the user's Claude Code) or "remove".
    action: String,
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
        "remove" | "enable" | "disable" => false,
        _ => return Err(bad("Hành động không hợp lệ")),
    };
    let name = input.name.as_str();
    let cdir = claude_dir(&state).await?;
    if let Some(on) = match input.action.as_str() {
        "enable" => Some(true),
        "disable" => Some(false),
        _ => None,
    } {
        return toggle(&state, &cdir, &input, on).await;
    }
    let output = match (input.target.as_str(), input.kind.as_str()) {
        ("mutis", "skill") if !add => {
            let config_path = state.config_path.clone();
            let name = name.to_owned();
            tokio::task::spawn_blocking(move || addons::remove_shared_skill(&config_path, &name))
                .await
                .map_err(|_| bad("Không xử lý được skill"))?
                .map_err(|error| bad(format!("{error:#}")))?;
            String::new()
        }
        ("claude", "skill") => {
            let hub = cdir.clone();
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
        ("claude", "plugin") => claude_plugin(&state, &cdir, name, add).await?,
        ("claude", "mcp") => claude_mcp(&state, &cdir, name, add).await?,
        ("codex", "plugin") => codex_plugin(&state, name, add).await?,
        ("antigravity", "plugin") => antigravity_plugin(&state, name, add).await?,
        _ => return Err(bad("Loại này chưa được hỗ trợ")),
    };
    Ok(Json(json!({"ok": true, "output": output})))
}

async fn toggle(state: &AppState, cdir: &Path, input: &Change, on: bool) -> ApiResult<Json<Value>> {
    let name = input.name.as_str();
    match (input.target.as_str(), input.kind.as_str()) {
        ("claude", "mcp") => {
            let mut config = state.config.write().await;
            let mut next = config.clone();
            next.claude.enabled_mcp.retain(|n| n != name);
            if on {
                next.claude.enabled_mcp.push(name.to_owned());
            }
            crate::config::save(&state.config_path, &next).map_err(|_| {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình")
            })?;
            *config = next;
            return Ok(Json(json!({"ok": true, "output": ""})));
        }
        ("codex", "mcp") => {
            let (_, home) = codex_cli(state).await?;
            crate::providers::codex::environment::set_mcp_enabled(&home, name, on)
                .map_err(|error| bad(format!("{error:#}")))?;
            return Ok(Json(json!({"ok": true, "output": ""})));
        }
        ("antigravity", "plugin") => {
            let (agy, home) = agy_cli(state).await?;
            let verb = if on { "enable" } else { "disable" };
            let output = run_cli_in(&agy, &["plugin", verb, name], false, Some(&home)).await?;
            return Ok(Json(json!({"ok": true, "output": output})));
        }
        ("claude", "plugin") => {}
        _ => return Err(bad("Chỉ bật/tắt được plugin của Claude, Antigravity và MCP")),
    }
    let claude = executable(state, "claude_cli", "claude").await?;
    let verb = if on { "enable" } else { "disable" };
    let output = run_cli_in(
        &claude,
        &["plugin", verb, name, "--scope", "user"],
        true,
        Some(cdir),
    )
    .await?;
    Ok(Json(json!({"ok": true, "output": output})))
}

async fn claude_plugin(
    state: &AppState,
    cdir: &Path,
    name: &str,
    add: bool,
) -> ApiResult<String> {
    let claude = executable(state, "claude_cli", "claude").await?;
    if add {
        // The plugin's marketplace must be known to AI Hub's Claude first; it is the one the user installed from.
        let personal = tokio::task::spawn_blocking(addons::personal_claude_home)
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
        let _ = run_cli_in(&claude, &["plugin", "marketplace", "add", &source], true, Some(cdir)).await;
        run_cli_in(&claude,
            &["plugin", "install", name, "--scope", "user"], true, Some(cdir))
        .await
    } else {
        run_cli_in(&claude,
            &["plugin", "uninstall", name, "--scope", "user"], true, Some(cdir))
        .await
    }
}

async fn claude_mcp(
    state: &AppState,
    cdir: &Path,
    name: &str,
    add: bool,
) -> ApiResult<String> {
    let claude = executable(state, "claude_cli", "claude").await?;
    if add {
        let name_owned = name.to_owned();
        let definition = tokio::task::spawn_blocking(move || {
            addons::personal_mcp_definition(&name_owned)
        })
        .await
        .map_err(|_| bad("Không đọc được MCP server"))?
        .map_err(|error| bad(error.to_string()))?;
        let json = definition.to_string();
        run_cli_in(&claude,
            &["mcp", "add-json", name, &json, "--scope", "user"], true, Some(cdir))
        .await
    } else {
        run_cli_in(&claude, &["mcp", "remove", name, "--scope", "user"], true, Some(cdir)).await
    }
}

async fn codex_plugin(state: &AppState, name: &str, add: bool) -> ApiResult<String> {
    let (codex, home) = codex_cli(state).await?;
    if add {
        // Copy from the user's own Codex: add its marketplace to AI Hub's Codex first when it has a source.
        let personal = codex_list(&codex, None).await;
        let plugin = personal
            .iter()
            .find(|plugin| plugin["pluginId"].as_str() == Some(name))
            .ok_or_else(|| bad(format!("Codex của bạn không có plugin «{name}»")))?;
        if let Some(source) = plugin["marketplaceSource"]["source"].as_str() {
            let source = source.trim_start_matches(r"\\?\");
            // An already-added or built-in marketplace makes this fail; the install below reports any real problem.
            let _ = run_cli_in(
                &codex,
                &["plugin", "marketplace", "add", source],
                false,
                Some(&home),
            )
            .await;
        }
        run_cli_in(&codex, &["plugin", "add", name], false, Some(&home)).await
    } else {
        run_cli_in(&codex, &["plugin", "remove", name], false, Some(&home)).await
    }
}

async fn antigravity_plugin(state: &AppState, name: &str, add: bool) -> ApiResult<String> {
    let (agy, home) = agy_cli(state).await?;
    if add {
        // Copy from the user's own Claude Code: agy imports a plugin folder with its skills and commands.
        let (_, path, _) = claude_plugin_paths()
            .into_iter()
            .find(|(plugin, ..)| plugin == name)
            .ok_or_else(|| bad(format!("Claude Code của bạn không có plugin «{name}»")))?;
        run_cli_in(&agy, &["plugin", "import", &path], false, Some(&home)).await
    } else {
        run_cli_in(&agy, &["plugin", "uninstall", name], false, Some(&home)).await
    }
}
