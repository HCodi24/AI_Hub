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
    /// "claude" | "codex" | "mutis".
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

/// The Claude config folder chats run with (`<data>/claude/<source id>`): plugins, MCP and skills must live there to be seen.
async fn claude_dir(state: &AppState) -> ApiResult<PathBuf> {
    let provider = state.config.read().await.providers.iter().find(|p| p.kind == "claude_cli").cloned();
    match provider {
        Some(provider) => crate::providers::claude::environment::home(&state.config_path, &provider)
            .map_err(|error| bad(error.to_string())),
        None => cli_support::ai_hub_claude_config_dir()
            .ok_or_else(|| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không tạo được thư mục cấu hình Claude của MutiS")),
    }
}

pub(crate) async fn overview(State(state): State<AppState>) -> ApiResult<Json<addons::Overview>> {
    let error = || {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được danh sách plugin và skill",
        )
    };
    let dir = claude_dir(&state).await.map_err(|_| error())?;
    let skills_dir = addons::shared_skills_dir(&state.config_path).map_err(|_| error())?;
    let mut overview = tokio::task::spawn_blocking(move || addons::overview(&dir, &skills_dir))
        .await
        .map_err(|_| error())?;
    let enabled = state.config.read().await.claude.enabled_mcp.clone();
    for server in &mut overview.hub.mcp { server.enabled = enabled.contains(&server.name); }
    Ok(Json(overview))
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
/// Runs a CLI and returns the last 4000 characters of its output (for messages).
/// `codex_home` runs a Codex command against AI Hub's own Codex home instead of `~/.codex`; for Claude it is the config folder.
async fn run_cli_in(
    path: &std::path::Path,
    args: &[&str],
    claude: bool,
    codex_home: Option<&std::path::Path>,
) -> ApiResult<String> {
    let text = run_cli_full(path, args, claude, codex_home).await?;
    let chars: Vec<char> = text.chars().collect();
    Ok(chars[chars.len().saturating_sub(4000)..].iter().collect())
}

/// Same, with the whole output: a JSON listing is cut and unreadable at 4000 characters.
async fn run_cli_full(
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
        if path.file_stem().is_some_and(|stem| stem.eq_ignore_ascii_case("agy")) {
            cli_support::agy_environment(&mut command, home);
        } else {
            command.env(if claude { "CLAUDE_CONFIG_DIR" } else { "CODEX_HOME" }, home);
        }
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
    let text = text.trim().to_owned();
    if !output.status.success() {
        let chars: Vec<char> = text.chars().collect();
        return Err(bad(if text.is_empty() {
            "Lệnh thất bại".to_owned()
        } else {
            chars[chars.len().saturating_sub(4000)..].iter().collect()
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
        match (input.target.as_str(), input.kind.as_str()) {
            ("claude", "mcp") => {
                let mut config = state.config.write().await;
                config.claude.enabled_mcp.retain(|n| n != name);
                if on { config.claude.enabled_mcp.push(name.to_owned()); }
                crate::config::save(&state.config_path, &config).map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không thể lưu cấu hình"))?;
                return Ok(Json(json!({"ok": true, "output": ""})));
            }
            ("codex", "mcp") => {
                let (_, home) = codex_cli(&state).await?;
                crate::providers::codex::environment::set_mcp_enabled(&home, name, on).map_err(|error| bad(format!("{error:#}")))?;
                return Ok(Json(json!({"ok": true, "output": ""})));
            }
            ("antigravity", "plugin") => {
                let (agy, home) = agy_cli(&state).await?;
                let output = run_cli_in(&agy, &["plugin", if on { "enable" } else { "disable" }, name], false, Some(&home)).await?;
                return Ok(Json(json!({"ok": true, "output": output})));
            }
            ("claude", "plugin") => {}
            _ => return Err(bad("Chỉ bật/tắt được plugin của Claude, Antigravity và MCP")),
        }
        let claude = executable(&state, "claude_cli", "claude").await?;
        let verb = if on { "enable" } else { "disable" };
        let output = run_cli_in(&claude, &["plugin", verb, name, "--scope", "user"], true, Some(&cdir)).await?;
        return Ok(Json(json!({"ok": true, "output": output})));
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
        ("claude", "plugin") => {
            let claude = executable(&state, "claude_cli", "claude").await?;
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
                let _ = run_cli_in(&claude, &["plugin", "marketplace", "add", &source], true, Some(&cdir)).await;
                run_cli_in(&claude,
                    &["plugin", "install", name, "--scope", "user"], true, Some(&cdir))
                .await?
            } else {
                run_cli_in(&claude,
                    &["plugin", "uninstall", name, "--scope", "user"], true, Some(&cdir))
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
                run_cli_in(&claude,
                    &["mcp", "add-json", name, &json, "--scope", "user"], true, Some(&cdir))
                .await?
            } else {
                run_cli_in(&claude, &["mcp", "remove", name, "--scope", "user"], true, Some(&cdir)).await?
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
        ("antigravity", "plugin") => {
            let (agy, home) = agy_cli(&state).await?;
            if add {
                // Copy from the user's own Claude Code: agy imports a plugin folder with its skills and commands.
                let (_, path, _) = claude_plugin_paths()
                    .into_iter()
                    .find(|(plugin, ..)| plugin == name)
                    .ok_or_else(|| bad(format!("Claude Code của bạn không có plugin «{name}»")))?;
                run_cli_in(&agy, &["plugin", "import", &path], false, Some(&home)).await?
            } else {
                run_cli_in(&agy, &["plugin", "uninstall", name], false, Some(&home)).await?
            }
        }
        _ => return Err(bad("Loại này chưa được hỗ trợ")),
    };
    Ok(Json(json!({"ok": true, "output": output})))
}

/// The Antigravity CLI and the clean home chats run with.
async fn agy_cli(state: &AppState) -> ApiResult<(PathBuf, PathBuf)> {
    let provider = state.config.read().await.providers.iter().find(|p| p.kind == "antigravity_cli").cloned()
        .ok_or_else(|| bad("MutiS chưa có nguồn Antigravity"))?;
    let agy = executable(state, "antigravity_cli", "agy").await?;
    let home = crate::providers::antigravity_cli::clean_home(&provider.id).map_err(|error| bad(error.to_string()))?;
    Ok((agy, home))
}

/// `(name, install folder, version)` of the plugins installed in the user's own Claude Code.
fn claude_plugin_paths() -> Vec<(String, String, String)> {
    let Some(file) = addons::personal_claude_dir().map(|dir| dir.join("plugins").join("installed_plugins.json")) else { return Vec::new() };
    let Some(json) = std::fs::read_to_string(file).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) else { return Vec::new() };
    json["plugins"].as_object().into_iter().flatten().filter_map(|(id, entries)| {
        let entry = entries.as_array()?.first()?;
        Some((id.split('@').next()?.to_owned(), entry["installPath"].as_str()?.to_owned(), entry["version"].as_str().unwrap_or_default().to_owned()))
    }).collect()
}

/// Plugins in AI Hub's Antigravity, and the Claude Code plugins that "Thêm" can import into it.
pub(crate) async fn antigravity_plugins(State(state): State<AppState>) -> Json<Value> {
    let Ok((agy, home)) = agy_cli(&state).await else { return Json(json!({"plugins": [], "available": []})) };
    let installed: Vec<Value> = match run_cli_full(&agy, &["plugin", "list", "--json"], false, Some(&home)).await {
        Ok(text) => text.find('[').and_then(|start| serde_json::from_str(&text[start..]).ok()).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    let names: Vec<&str> = installed.iter().filter_map(|plugin| plugin["name"].as_str()).collect();
    let available: Vec<Value> = claude_plugin_paths().into_iter()
        .filter(|(name, ..)| !names.contains(&name.as_str()))
        .map(|(name, _, version)| json!({"id": name, "version": version, "enabled": true}))
        .collect();
    let plugins: Vec<Value> = installed.iter().filter_map(|plugin| Some(json!({
        "id": plugin["name"].as_str()?, "version": plugin["version"].as_str().unwrap_or_default(),
        "enabled": plugin["enabled"].as_bool().unwrap_or(true),
    }))).collect();
    Json(json!({"plugins": plugins, "available": available}))
}

async fn codex_provider(state: &AppState) -> ApiResult<crate::config::ProviderConfig> {
    state
        .config
        .read()
        .await
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .cloned()
        .ok_or_else(|| bad("MutiS chưa có nguồn Codex"))
}

/// The Codex CLI and AI Hub's own Codex home (never `~/.codex`).
async fn codex_cli(state: &AppState) -> ApiResult<(PathBuf, PathBuf)> {
    let provider = codex_provider(state).await?;
    let codex = executable(state, "codex_cli", "codex").await?;
    let config = crate::config::config_file_path().map_err(|error| bad(error.to_string()))?;
    let home = crate::providers::codex::environment::home(&config, &provider)
        .map_err(|error| bad(error.to_string()))?;
    Ok((codex, home))
}

/// `codex plugin list --json` -> its `installed` entries; `home` picks AI Hub's Codex, `None` the user's own.
/// Any failure (no Codex, no plugins) is an empty list.
async fn codex_list(codex: &std::path::Path, home: Option<&std::path::Path>) -> Vec<Value> {
    let Ok(text) = run_cli_full(codex, &["plugin", "list", "--json"], false, home).await else {
        return Vec::new();
    };
    // Codex may print a warning line before the JSON.
    text.find('{')
        .and_then(|start| serde_json::from_str::<Value>(&text[start..]).ok())
        .and_then(|json| json["installed"].as_array().cloned())
        .unwrap_or_default()
}

async fn claude_hub_has_plugin(state: &AppState, id: &str) -> bool {
    let Ok(hub_dir) = claude_dir(state).await else {
        return false;
    };
    let Ok(skills_dir) = addons::shared_skills_dir(&state.config_path) else {
        return false;
    };
    let id = id.to_owned();
    tokio::task::spawn_blocking(move || {
        addons::overview(&hub_dir, &skills_dir)
            .hub
            .plugins
            .iter()
            .any(|plugin| plugin.id == id)
    })
    .await
    .unwrap_or(false)
}

/// Marketplace of OpenAI's account-wide plugins: every ChatGPT login gets them and they cannot be uninstalled.
const CODEX_DEFAULT_MARKETPLACE: &str = "openai-curated-remote";

/// True for a plugin Codex installs on its own for the signed-in account (an explicit flag when Codex reports one,
/// otherwise its marketplace).
fn codex_default_plugin(plugin: &Value) -> bool {
    let flagged = ["installedByDefault", "installed_by_default", "isDefault", "default"]
        .iter()
        .any(|key| plugin[*key].as_bool() == Some(true));
    flagged
        || plugin["pluginId"]
            .as_str()
            .and_then(|id| id.rsplit_once('@'))
            .is_some_and(|(_, market)| market == CODEX_DEFAULT_MARKETPLACE || market.starts_with("openai-bundled") || market == "openai-primary-runtime")
}

/// Plugins in AI Hub's Codex, and the ones only the user's own Codex has (what "Thêm" can copy over).
pub(crate) async fn codex_plugins(State(state): State<AppState>) -> Json<Value> {
    let (mut installed, mut available) = (Vec::new(), Vec::new());
    if let Ok((codex, home)) = codex_cli(&state).await {
        installed = codex_list(&codex, Some(&home)).await;
        // `codex plugin list` can come back short on another machine; the cache folder is the ground truth.
        for cached in crate::providers::codex::environment::cached_plugins(&home) {
            if !installed.iter().any(|p| p["pluginId"] == cached["pluginId"]) { installed.push(cached); }
        }
        available = codex_list(&codex, None).await;
    }
    // Plugins the account installs by default cannot be removed (the server answers 403), so they are not listed.
    installed.retain(|plugin| !codex_default_plugin(plugin));
    available.retain(|plugin| !codex_default_plugin(plugin));
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
    let mcp = match codex_cli(&state).await {
        Ok((_, home)) => crate::providers::codex::environment::mcp_servers(&home),
        Err(_) => Vec::new(),
    };
    Json(json!({ "plugins": view(&installed), "available": view(&available), "mcp": mcp }))
}

/// Removes the downloaded repository when dropped, however the request ends.
struct TempCheckout(PathBuf);

impl TempCheckout {
    async fn git_output(&self, args: &[&str]) -> ApiResult<Vec<u8>> {
        // Clone without a worktree first, then materialize it without user/system filters or hooks.
        let mut git = checkout_git();
        git.current_dir(&self.0)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", self.0.join(".git/mutis-empty-config"))
            .env_remove("GIT_CONFIG_COUNT")
            .env_remove("GIT_CONFIG_PARAMETERS")
            .args(args);
        let output = crate::core::process::run_with_timeout(&mut git, Duration::from_secs(30))
            .await
            .map_err(|error| bad(format!("Không đọc được bản sao kho mã: {error}")))?;
        if !output.status.success() {
            return Err(bad("Không đọc được bản sao kho mã"));
        }
        Ok(output.stdout)
    }
}

impl Drop for TempCheckout {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn checkout_git() -> tokio::process::Command {
    let mut git = tokio::process::Command::new("git");
    git.env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .kill_on_drop(true);
    for name in [
        "GIT_DIR",
        "GIT_COMMON_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        git.env_remove(name);
    }
    crate::core::process::configure_hidden(&mut git);
    git
}

struct Checkout {
    source: String,
    dir: TempCheckout,
    found: addons::LinkFound,
    tree: Vec<u8>,
}

/// Download and inspect a shallow GitHub checkout.
async fn checkout(link: &str) -> ApiResult<Checkout> {
    let source = addons::github_source(link).map_err(|error| bad(error.to_string()))?;
    let temp = std::env::temp_dir()
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|error| bad(format!("Không tìm được thư mục tạm: {error}")))?;
    let path = temp.join(format!("mutis-addon-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&path)
        .map_err(|error| bad(format!("Không tạo được thư mục tạm: {error}")))?;
    let dir = TempCheckout(path);
    let mut git = checkout_git();
    git.args([
        "clone",
        "--depth",
        "1",
        "--no-checkout",
        "--template=",
        "--",
        &source,
    ])
    .arg(&dir.0);
    let output = crate::core::process::run_with_timeout(&mut git, Duration::from_secs(90))
        .await
        .map_err(|error| bad(format!("Không tải được repository: {error}")))?;
    if !output.status.success() {
        return Err(bad(
            "Không tải được repository (link sai, repository riêng tư hoặc máy không có git)",
        ));
    }
    dir.git_output(&["checkout", "--detach", "HEAD"]).await?;
    let tree = dir.git_output(&["ls-tree", "-r", "-z", "HEAD"]).await?;
    let found = addons::inspect_checkout(&dir.0, &source, &tree);
    Ok(Checkout {
        source,
        dir,
        found,
        tree,
    })
}

#[derive(Deserialize)]
pub(crate) struct LinkInspect {
    link: String,
}

pub(crate) async fn inspect_link(Json(input): Json<LinkInspect>) -> ApiResult<Json<Value>> {
    let Checkout { source, found, .. } = checkout(&input.link).await?;
    Ok(Json(json!({
        "source": source,
        "claude": found.claude,
        "codex": found.codex,
        "skills": found.skills,
    })))
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
    let Checkout { source, found, .. } = checkout(&input.link).await?;
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
            (executable(&state, "claude_cli", "claude").await?, Some(claude_dir(&state).await?))
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
                Ok(output) => {
                    let installed = if claude {
                        claude_hub_has_plugin(&state, &id).await
                    } else {
                        codex_list(&cli, home.as_deref())
                            .await
                            .iter()
                            .any(|plugin| plugin["pluginId"].as_str() == Some(id.as_str()))
                    };
                    if installed {
                        log.push(format!("{target}: đã cài {id}"));
                    } else {
                        failed = true;
                        log.push(format!(
                            "{target}: CLI báo xong nhưng {id} chưa có trong danh sách. Kết quả CLI: {output}"
                        ));
                    }
                }
                Err(error) => {
                    failed = true;
                    log.push(format!("{target}: lỗi {id}: {}", error.message));
                }
            }
        }
    }
    Ok(Json(json!({ "ok": !failed, "output": log.join("\n") })))
}

#[derive(Deserialize)]
pub(crate) struct LinkSkillInstall {
    link: String,
    skill_id: String,
    confirmation: Option<String>,
}

pub(crate) async fn install_link_skill(
    State(state): State<AppState>,
    Json(input): Json<LinkSkillInstall>,
) -> ApiResult<Json<Value>> {
    if input.skill_id.len() != 64 || !input.skill_id.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(bad("Mã skill không hợp lệ. Hãy kiểm tra lại liên kết."));
    }
    let Checkout {
        dir, found, tree, ..
    } = checkout(&input.link).await?;
    let skill = found
        .skills
        .into_iter()
        .find(|skill| skill.id == input.skill_id)
        .ok_or_else(|| bad("Không tìm thấy skill đã chọn. Hãy kiểm tra lại liên kết."))?;
    let name = skill.name.clone();
    let confirmation = input.confirmation;
    let skill_id = skill.id.clone();
    let config_path = state.config_path.clone();
    let result = tokio::task::spawn_blocking(move || {
        addons::install_checkout_skill(&dir.0, &tree, &skill, &config_path, confirmation.as_deref())
    })
    .await
    .map_err(|_| bad("Không xử lý được skill"))?
    .map_err(|error| bad(format!("{error:#}")))?;
    Ok(Json(match result {
        addons::SkillInstall::Installed => json!({
            "ok": true,
            "output": format!("Đã cài skill «{name}» vào kho chung của MutiS. Gõ /{name} ở đầu tin nhắn để dùng."),
        }),
        addons::SkillInstall::Conflict(confirmation) => json!({
            "ok": false,
            "output": format!("Kho chung của MutiS đã có skill «{name}». Cần xác nhận để thay thế."),
            "conflict": { "skill_id": skill_id, "confirmation": confirmation },
        }),
    }))
}

#[cfg(test)]
mod tests {
    use super::codex_default_plugin;
    use serde_json::json;

    #[test]
    fn default_codex_plugins_are_detected() {
        assert!(codex_default_plugin(&json!({"pluginId": "sites@openai-curated-remote"})));
        assert!(codex_default_plugin(&json!({"pluginId": "x@market", "installedByDefault": true})));
        assert!(!codex_default_plugin(&json!({"pluginId": "x@openai-curated"})));
        assert!(!codex_default_plugin(&json!({"pluginId": "x@my-market"})));
    }
}
