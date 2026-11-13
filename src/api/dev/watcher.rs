//! Background watcher that notices Rust/web changes and optionally auto-rebuilds.

use super::build::{npm_build_command, run_dev_build};
use crate::app::AppState;
use serde_json::json;
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
    time::{Duration, Instant},
};

pub(crate) fn spawn_dev_watcher(state: AppState, repo: PathBuf) {
    tokio::spawn(async move {
        let mut old_rust = crate::dev::snapshot(&repo);
        let mut old_web = crate::dev::snapshot_web_src(&repo);
        let mut old_out = crate::dev::snapshot_web_out(&repo);
        let mut out_changed_at: Option<Instant> = None;
        let mut pending: Option<Instant> = None;
        let mut pending_web: Option<Instant> = None;
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let rust = crate::dev::snapshot(&repo);
            let rust_changes = crate::dev::changed_files(&old_rust, &rust);
            let web = crate::dev::snapshot_web_src(&repo);
            let web_changes = crate::dev::changed_files(&old_web, &web);
            if !web_changes.is_empty() {
                let _ = state.dev_events.send(
                    json!({"event":"web_changed","data":{"files":web_changes.len()}}).to_string(),
                );
                pending_web = Some(Instant::now() + Duration::from_secs(3));
            }
            old_web = web;
            let out = crate::dev::snapshot_web_out(&repo);
            if !crate::dev::changed_files(&old_out, &out).is_empty() {
                out_changed_at = Some(Instant::now());
            }
            old_out = out;
            // Reload the page only once the new export is finished: unchanged for 3 seconds and index.html is back.
            if out_changed_at.is_some_and(|at| at.elapsed() >= Duration::from_secs(3))
                && repo.join("web/out/index.html").is_file()
            {
                out_changed_at = None;
                let _ = state
                    .dev_events
                    .send(json!({"event":"ui_changed","data":{}}).to_string());
            }
            if !rust_changes.is_empty() {
                let files = rust_changes
                    .iter()
                    .map(|p| {
                        p.strip_prefix(&repo)
                            .unwrap_or(p)
                            .to_string_lossy()
                            .to_string()
                    })
                    .collect::<Vec<_>>();
                let _ = state
                    .dev_events
                    .send(json!({"event":"rust_changed","data":{"files":files}}).to_string());
                pending = Some(Instant::now() + Duration::from_secs(3));
            }
            old_rust = rust;
            // Auto-rebuild also covers the UI: without it a web edit never shows until someone clicks "Build giao diện".
            if pending_web.is_some_and(|at| Instant::now() >= at) {
                pending_web = None;
                if state.config.read().await.self_dev.auto_rebuild
                    && state.active_jobs.load(Ordering::SeqCst) == 0
                {
                    let _ = npm_build_command(&repo).output().await;
                }
            }
            if pending.is_some_and(|at| Instant::now() >= at) {
                pending = None;
                if state.config.read().await.self_dev.auto_rebuild
                    && state.active_jobs.load(Ordering::SeqCst) == 0
                {
                    let _ = run_dev_build(&state, &repo).await;
                }
            }
        }
    });
}
