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
    /// "claude" (AI Hub's Claude Code) or "codex" (AI Hub's Codex).
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
    run_cli_in(path, args, claude, None).await
}

/// Like `run_cli`; `codex_home` runs a Codex command against AI Hub's own Codex home instead of `~/.codex`.
async fn run_cli_in(
    path: &std::path::Path,
    args: &[&str],
    claude: bool,
    codex_home: Option<&std::path::Path>,
) -> ApiResult<String> {
    let args: Vec<OsString> = args.iter().map(OsString::from).collect();
    let mut command = cli_support::command_for(path, &args);
    if claude {
        cli_support::clean_claude_environment(&mut command);
    }
    if let Some(home) = codex_home {
        command.env("CODEX_HOME", home);
    }
    crate::core::process::configure_hidden(&mut command);
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
        ("codex", "plugin") => {
            let (codex, home) = codex_cli(&state).await?;
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
                run_cli_in(&codex, &["plugin", "add", name], false, Some(&home)).await?
            } else {
                run_cli_in(&codex, &["plugin", "remove", name], false, Some(&home)).await?
            }
        }
        _ => return Err(bad("Loại này chưa được hỗ trợ")),
    };
    Ok(Json(json!({"ok": true, "output": output})))
}

/// The Codex CLI and AI Hub's own Codex home (never `~/.codex`).
async fn codex_cli(state: &AppState) -> ApiResult<(PathBuf, PathBuf)> {
    let provider = state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .cloned()
        .ok_or_else(|| bad("MutiS chưa có nguồn Codex"))?;
    let codex = executable(state, "codex_cli", "codex").await?;
    let config = crate::config::config_file_path().map_err(|error| bad(error.to_string()))?;
    let home = crate::providers::codex::environment::home(&config, &provider)
        .map_err(|error| bad(error.to_string()))?;
    Ok((codex, home))
}

/// `codex plugin list --json` -> its `installed` entries; `home` picks AI Hub's Codex, `None` the user's own.
/// Any failure (no Codex, no plugins) is an empty list.
async fn codex_list(codex: &std::path::Path, home: Option<&std::path::Path>) -> Vec<Value> {
    let Ok(text) = run_cli_in(codex, &["plugin", "list", "--json"], false, home).await else {
        return Vec::new();
    };
    // Codex may print a warning line before the JSON.
    text.find('{')
        .and_then(|start| serde_json::from_str::<Value>(&text[start..]).ok())
        .and_then(|json| json["installed"].as_array().cloned())
        .unwrap_or_default()
}

/// Plugins in AI Hub's Codex, and the ones only the user's own Codex has (what "Thêm" can copy over).
pub(crate) async fn codex_plugins(State(state): State<AppState>) -> Json<Value> {
    let (mut installed, mut available) = (Vec::new(), Vec::new());
    if let Ok((codex, home)) = codex_cli(&state).await {
        installed = codex_list(&codex, Some(&home)).await;
        available = codex_list(&codex, None).await;
    }
    let view = |plugins: &[Value]| -> Vec<Value> {
        plugins
            .iter()
            .filter_map(|plugin| {
                Some(json!({
                    "id": plugin["pluginId"].as_str()?,
                    "version": plugin["version"].as_str().unwrap_or_default(),
                    "enabled": plugin["enabled"].as_bool().unwrap_or(true),
                }))
            })
            .collect()
    };
    let have: Vec<&str> = installed
        .iter()
        .filter_map(|plugin| plugin["pluginId"].as_str())
        .collect();
    available.retain(|plugin| {
        plugin["pluginId"]
            .as_str()
            .is_some_and(|id| !have.contains(&id))
    });
    Json(json!({ "plugins": view(&installed), "available": view(&available) }))
}

/// Removes the downloaded repository when dropped, however the request ends.
struct TempCheckout(PathBuf);

impl Drop for TempCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Downloads a GitHub repository (depth 1) and says which CLIs it can be installed into. Nothing is run from it.
async fn checkout(link: &str) -> ApiResult<(String, TempCheckout, addons::LinkFound)> {
    let source = addons::github_source(link).map_err(|error| bad(error.to_string()))?;
    let dir =
        TempCheckout(std::env::temp_dir().join(format!("mutis-plugin-{}", uuid::Uuid::new_v4())));
    let mut git = tokio::process::Command::new("git");
    git.args(["clone", "--depth", "1", "--", &source])
        .arg(&dir.0)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    crate::core::process::configure_hidden(&mut git);
    let output = crate::core::process::run_with_timeout(&mut git, Duration::from_secs(90))
        .await
        .map_err(|error| bad(format!("Không tải được repository: {error}")))?;
    if !output.status.success() {
        return Err(bad(
            "Không tải được repository (link sai, repository riêng tư hoặc máy không có git)",
        ));
    }
    let found = addons::inspect_checkout(&dir.0);
    Ok((source, dir, found))
}

#[derive(Deserialize)]
pub(crate) struct LinkInspect {
    link: String,
}

pub(crate) async fn inspect_link(Json(input): Json<LinkInspect>) -> ApiResult<Json<Value>> {
    let (source, _dir, found) = checkout(&input.link).await?;
    Ok(Json(
        json!({ "source": source, "claude": found.claude, "codex": found.codex }),
    ))
}

#[derive(Deserialize)]
pub(crate) struct LinkInstall {
    link: String,
    #[serde(default)]
    targets: Vec<String>,
    #[serde(default)]
    plugins: Vec<String>,
}

/// Installs the chosen plugins of a GitHub marketplace into AI Hub's own Claude and/or Codex. The marketplace and
/// plugin names come from the downloaded files, never from the client.
pub(crate) async fn install_link(
    State(state): State<AppState>,
    Json(input): Json<LinkInstall>,
) -> ApiResult<Json<Value>> {
    let (source, _dir, found) = checkout(&input.link).await?;
    let mut log = Vec::new();
    let mut failed = false;
    for target in &input.targets {
        let (market, claude) = match target.as_str() {
            "claude" => (found.claude.as_ref(), true),
            "codex" => (found.codex.as_ref(), false),
            _ => return Err(bad("Chỉ cài được cho claude hoặc codex")),
        };
        let market =
            market.ok_or_else(|| bad(format!("Repository này không có plugin cho {target}")))?;
        let names: Vec<&str> = input
            .plugins
            .iter()
            .map(String::as_str)
            .filter(|name| market.plugins.iter().any(|plugin| plugin.name == *name))
            .collect();
        if names.is_empty() {
            return Err(bad(format!("Chưa chọn plugin nào cho {target}")));
        }
        let (cli, home) = if claude {
            (executable(&state, "claude_cli", "claude").await?, None)
        } else {
            let (codex, home) = codex_cli(&state).await?;
            (codex, Some(home))
        };
        // An already-added marketplace makes this fail; the install below reports any real problem.
        let _ = run_cli_in(
            &cli,
            &["plugin", "marketplace", "add", &source],
            claude,
            home.as_deref(),
        )
        .await;
        for name in names {
            let id = format!("{name}@{}", market.name);
            let args: Vec<&str> = if claude {
                vec!["plugin", "install", &id, "--scope", "user"]
            } else {
                vec!["plugin", "add", &id]
            };
            match run_cli_in(&cli, &args, claude, home.as_deref()).await {
                Ok(_) => log.push(format!("{target}: đã cài {id}")),
                Err(error) => {
                    failed = true;
                    log.push(format!("{target}: lỗi {id}: {}", error.message));
                }
            }
        }
    }
    Ok(Json(json!({ "ok": !failed, "output": log.join("\n") })))
}
