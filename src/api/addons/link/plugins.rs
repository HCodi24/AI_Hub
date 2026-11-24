use super::super::{
    bad, claude_dir,
    cli::run_cli_in,
    codex::{codex_cli, codex_list},
    executable,
};
use super::checkout::{checkout, Checkout};
use crate::{api::ApiResult, app::AppState, features::addons};
use axum::{extract::State, Json};
use serde::Deserialize;
use serde_json::{json, Value};

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
        "mcp": found.mcp,
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
            (
                executable(&state, "claude_cli", "claude").await?,
                Some(claude_dir(&state).await?),
            )
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
