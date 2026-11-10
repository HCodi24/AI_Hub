use crate::{
    api::{ApiError, ApiResult},
    app::{AppState, JobGuard},
    chat::{self, ChatMessage},
    plans::{Plan, PlanTask},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{sse::KeepAlive, sse::Sse, IntoResponse, Response},
    Json,
};
use futures_util::{future::join_all, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{convert::Infallible, path::PathBuf};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize)]
pub(super) struct PlanRequest {
    request: String,
    folder: String,
    #[serde(default)]
    dev_chat: bool,
    #[serde(default)]
    provider_id: String,
    #[serde(default)]
    model: String,
}

struct CancelOnDrop(CancellationToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(super) async fn list_plans(State(state): State<AppState>) -> ApiResult<Json<Vec<Plan>>> {
    state
        .plans
        .list()
        .map(Json)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được kế hoạch"))
}
pub(super) async fn get_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Plan>> {
    state
        .plans
        .get(&id)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không đọc được kế hoạch"))?
        .map(Json)
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy kế hoạch"))
}
pub(super) async fn save_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(mut plan): Json<Plan>,
) -> ApiResult<Json<Plan>> {
    if plan.id != id || !chat::valid_id(&id) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Mã kế hoạch không hợp lệ",
        ));
    }
    if plan.tasks.iter().any(|t| {
        t.title.trim().is_empty()
            || t.detail.trim().is_empty()
            || !matches!(
                t.kind.as_str(),
                "plan" | "small_edit" | "code" | "test" | "docs" | "review"
            )
    }) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Công việc chưa hợp lệ",
        ));
    }
    if plan.approved && crate::plans::has_dependency_cycle(&plan.tasks) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Phụ thuộc công việc có chu trình",
        ));
    }
    plan.touch();
    state
        .plans
        .save(&plan)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được kế hoạch"))?;
    Ok(Json(plan))
}
pub(super) async fn generate_plan(
    State(state): State<AppState>,
    Json(input): Json<PlanRequest>,
) -> ApiResult<Response> {
    let requested_folder = if input.dev_chat && state.dev_repo.is_some() {
        PathBuf::from(crate::api::dev::effective_chat_folder(
            true,
            true,
            &input.folder,
            &crate::api::dev::dev_repo_path(&state)?.to_string_lossy(),
        ))
    } else {
        PathBuf::from(input.folder.trim())
    };
    let folder = requested_folder
        .canonicalize()
        .map(crate::core::text::strip_verbatim)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "Thư mục làm việc không tồn tại"))?;
    let cfg = state.config.read().await.clone();
    let statuses = join_all(
        cfg.providers
            .iter()
            .map(|p| crate::api::providers::cached_provider_test(&state, &p.id)),
    )
    .await;
    let connected =
        cfg.providers
            .iter()
            .zip(statuses)
            .filter_map(|(p, status)| {
                status.ok().filter(|s| s.status == "connected").map(|_| {
                    crate::plans::ProviderSummary {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        kind: p.kind.clone(),
                        models: p.models.clone(),
                        agent_capable: p.agent_capable,
                    }
                })
            })
            .collect::<Vec<_>>();
    let requested = if !input.provider_id.is_empty() {
        input.provider_id.clone()
    } else if !cfg.planner_provider_id.is_empty() {
        cfg.planner_provider_id.clone()
    } else {
        ["claude_cli", "anthropic", "codex_cli"]
            .iter()
            .find_map(|kind| {
                connected
                    .iter()
                    .find(|p| p.kind == *kind)
                    .map(|p| p.id.clone())
            })
            .unwrap_or_default()
    };
    let planner = cfg
        .providers
        .iter()
        .find(|p| p.id == requested)
        .cloned()
        .filter(|p| connected.iter().any(|w| w.id == p.id))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "Hãy chọn nguồn AI lập kế hoạch đã kết nối trong Cài đặt",
            )
        });
    let planner = match planner {
        Ok(planner) => planner,
        Err(error) if cfg.mutis.enabled => {
            let connected_ids = connected
                .iter()
                .map(|provider| provider.id.clone())
                .collect();
            crate::config::first_connected_fallback(&cfg.providers, "lead", &connected_ids)
                .and_then(|id| cfg.providers.iter().find(|item| item.id == id))
                .cloned()
                .ok_or(error)?
        }
        Err(error) => return Err(error),
    };
    let model = if planner.id != requested {
        planner.models.first().cloned().unwrap_or_default()
    } else if input.model.trim().is_empty() {
        if cfg.planner_model.is_empty() {
            planner.models.first().cloned().unwrap_or_default()
        } else {
            cfg.planner_model.clone()
        }
    } else {
        input.model.clone()
    };
    if model.trim().is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Nguồn lập kế hoạch cần có model",
        ));
    }
    let workers = connected
        .iter()
        .map(|p| {
            format!(
                "id: {}, name: {}, kind: {}, models: {}{}",
                p.id,
                p.name,
                p.kind,
                p.models.join(", "),
                if p.kind == "openai_compat" && p.agent_capable {
                    ", agent mode: yes (reads/edits files in the folder, never runs commands)"
                } else {
                    ""
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let routes = if cfg.mutis.enabled {
        format!(
            "\nMutiS routing table (kind -> role):\n{}\nRoles: lead={}, big={}, small={}. \
             Use a source marked agent mode only for small tasks; it never runs commands.",
            cfg.mutis
                .routing
                .iter()
                .map(|route| format!("{} -> {}", route.kind, route.role))
                .collect::<Vec<_>>()
                .join("\n"),
            cfg.mutis.roles.lead.provider_id,
            cfg.mutis.roles.big.provider_id,
            cfg.mutis.roles.small.provider_id
        )
    } else {
        String::new()
    };
    let request_context = if input.dev_chat {
        format!(
            "{}\n\n",
            crate::api::dev::dev_chat_preamble(&folder.to_string_lossy())
        )
    } else {
        String::new()
    };
    let prompt = format!(
        concat!(
            "Respond ONLY as JSON: {{\"title\":\"...\",\"tasks\":[",
            "{{\"id\":1,\"title\":\"...\",\"detail\":\"files + expected result\",",
            "\"assignee\":\"provider id\",\"depends_on\":[ids],",
            "\"kind\":\"plan|small_edit|code|test|docs|review\"}}]}}. ",
            "Keep tasks small and avoid editing same file in parallel.\n",
            "Request:\n{}{}\nWorking folder: {}\nFiles:\n{}\n",
            "Connected workers (id, name, kind, models):\n{}{}"
        ),
        request_context,
        input.request,
        folder.display(),
        file_tree(&folder),
        workers,
        routes
    );
    let provider = providers::for_kind(&planner.kind).map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "Loại nguồn AI lập kế hoạch không hợp lệ",
        )
    })?;
    let cancel = CancellationToken::new();
    let (out_tx, out_rx) = mpsc::channel(32);
    let (event_tx, mut event_rx) = mpsc::channel(32);
    let request = ProviderRequest {
        provider: planner,
        model,
        folder: folder.to_string_lossy().into_owned(),
        permission: chat::Permission::Read,
        effort: chat::Effort::Default,
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            error: None,
        }],
        session_id: None,
        mutis: cfg.mutis.clone(),
    };
    let task_cancel = cancel.clone();
    let job_guard = JobGuard::new(state.active_jobs.clone());
    let task_state = state.clone();
    let planner_id = request.provider.id.clone();
    let request_text = input.request.clone();
    let folder_text = folder.to_string_lossy().into_owned();
    tokio::spawn(async move {
        let _job_guard = job_guard;
        let run_cancel = task_cancel.clone();
        let tx = event_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = provider.run(request, tx.clone(), run_cancel).await {
                let _ = tx.send(ProviderEvent::text("error", e.to_string())).await;
            }
        });
        // Drop our sender so `event_rx.recv()` returns None once the provider task finishes.
        drop(event_tx);
        let mut raw = String::new();
        let mut error = None;
        loop {
            tokio::select! {
                _ = task_cancel.cancelled() => return,
                _ = out_tx.closed() => { task_cancel.cancel(); return; },
                event = event_rx.recv() => {
                    let Some(event) = event else { break };
                    if event.name == "delta" {
                        raw.push_str(event.data["text"].as_str().unwrap_or_default());
                    } else if event.name == "error" {
                        error = Some(
                            event.data["text"]
                                .as_str()
                                .unwrap_or("Nguồn lập kế hoạch gặp lỗi")
                                .to_owned(),
                        );
                    } else if event.name == "log"
                        && out_tx.send(chat::sse_event("log", &event.data)).await.is_err()
                    {
                        task_cancel.cancel();
                        return;
                    }
                }
            }
        }
        let result =
            (|| -> Result<Plan, String> {
                if let Some(e) = error {
                    return Err(e);
                };
                let parsed = crate::core::text::extract_json(&raw)
                    .and_then(|v| serde_json::from_str::<Value>(v).ok())
                    .ok_or_else(|| format!("Không đọc được JSON từ nguồn lập kế hoạch: {raw}"))?;
                let source = parsed["tasks"]
                    .as_array()
                    .ok_or_else(|| "Kết quả lập kế hoạch thiếu danh sách công việc".to_owned())?;
                let tasks =
                    source
                        .iter()
                        .enumerate()
                        .map(|(i, t)| {
                            let old = t["assignee"].as_str().unwrap_or(&planner_id).to_owned();
                            let kind = t["kind"].as_str().unwrap_or("code").to_owned();
                            let valid = connected.iter().any(|p| p.id == old);
                            let assignee = if cfg.mutis.enabled {
                                crate::plans::reassign_with_routing(
                                    &kind,
                                    &connected,
                                    &planner_id,
                                    &cfg.mutis.routing,
                                    &cfg.mutis.roles,
                                    cfg.mutis.enabled,
                                )
                            } else if valid {
                                old.clone()
                            } else {
                                crate::plans::reassign_with_routing(
                                    &kind,
                                    &connected,
                                    &planner_id,
                                    &cfg.mutis.routing,
                                    &cfg.mutis.roles,
                                    false,
                                )
                            };
                            let role = cfg.mutis.routing.iter().find(|rule| rule.kind == kind).map(
                                |rule| match rule.role.as_str() {
                                    "big" => &cfg.mutis.roles.big,
                                    "small" => &cfg.mutis.roles.small,
                                    _ => &cfg.mutis.roles.lead,
                                },
                            );
                            let assigned_provider =
                                connected.iter().find(|provider| provider.id == assignee);
                            let routed_role = role.filter(|role| role.provider_id == assignee);
                            let reassigned_from = if assignee == old { None } else { Some(old) };
                            PlanTask {
                                id: t["id"].as_u64().unwrap_or(i as u64 + 1) as u32,
                                title: t["title"].as_str().unwrap_or("Task").into(),
                                detail: t["detail"].as_str().unwrap_or("").into(),
                                assignee,
                                model: routed_role
                                    .map(|role| role.model.clone())
                                    .or_else(|| t["model"].as_str().map(str::to_owned))
                                    .filter(|model| !model.is_empty())
                                    .or_else(|| {
                                        assigned_provider
                                            .and_then(|provider| provider.models.first().cloned())
                                    }),
                                effort: routed_role.map(|role| role.effort.clone()).unwrap_or_else(
                                    || t["effort"].as_str().unwrap_or("default").into(),
                                ),
                                reassigned_from,
                                depends_on: t["depends_on"]
                                    .as_array()
                                    .into_iter()
                                    .flatten()
                                    .filter_map(Value::as_u64)
                                    .map(|v| v as u32)
                                    .collect(),
                                kind,
                                status: "chờ".into(),
                                summary: String::new(),
                                log: Vec::new(),
                                tokens: 0,
                                chat_id: None,
                                verify: None,
                                review: None,
                                fix_rounds_used: 0,
                                fix_root_title: None,
                                fix_root_detail: None,
                            }
                        })
                        .collect();
                Ok(Plan::new(
                    parsed["title"].as_str().unwrap_or("New plan").into(),
                    folder_text,
                    request_text,
                    tasks,
                )
                .with_dev_chat(input.dev_chat))
            })();
        match result {
            Ok(plan) => match task_state.plans.save(&plan) {
                Ok(()) => {
                    let _ = out_tx.send(chat::sse_event("plan", &json!(plan))).await;
                }
                Err(e) => {
                    let _ = out_tx
                        .send(chat::sse_event("error", &json!({"message":e.to_string()})))
                        .await;
                }
            },
            Err(e) => {
                let _ = out_tx
                    .send(chat::sse_event("error", &json!({"message":e})))
                    .await;
            }
        }
    });
    let stream_cancel = cancel.clone();
    let stream = async_stream::stream! {let _guard=CancelOnDrop(stream_cancel);let mut rx=ReceiverStream::new(out_rx);while let Some(event)=rx.next().await{yield Ok::<_,Infallible>(event)}};
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
#[cfg(test)]
mod plan_tests {
    use serde_json::Value;

    #[test]
    fn extracts_json_from_planner_answer_with_surrounding_text() {
        let answer = "Here is the plan:\n```json\n{\"title\":\"Ship it\",\"tasks\":[{\"id\":1}]}\n```\nDone.";
        let extracted =
            crate::core::text::extract_json(answer).expect("JSON object should be extracted");
        let value: Value = serde_json::from_str(extracted).expect("extracted JSON should parse");
        assert_eq!(value["title"], "Ship it");
        assert_eq!(value["tasks"][0]["id"], 1);
    }
}
fn file_tree(root: &std::path::Path) -> String {
    fn walk(dir: &std::path::Path, root: &std::path::Path, out: &mut Vec<String>) {
        if out.len() >= 200 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                if out.len() >= 200 {
                    break;
                }
                let path = entry.path();
                let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
                if path.is_dir()
                    && [".git", "target", "node_modules", ".codegraph"]
                        .iter()
                        .any(|name| path.file_name().is_some_and(|n| n == *name))
                {
                    continue;
                }
                out.push(if path.is_dir() {
                    format!("{rel}/")
                } else {
                    rel.to_string()
                });
                if path.is_dir() {
                    walk(&path, root, out);
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.join("\n")
}

/// `canonicalize()` on Windows returns verbatim paths like `\?\C:\TOOL\ai-hub`. cmd.exe cannot use those as the
/// current directory (and they look odd in the UI), so convert them back to normal paths.
#[cfg(test)]
mod strip_verbatim_tests {
    use crate::core::text::strip_verbatim;
    use std::path::PathBuf;

    #[test]
    fn strips_windows_verbatim_prefixes() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\TOOL\ai-hub")),
            PathBuf::from(r"C:\TOOL\ai-hub")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\srv\share")),
            PathBuf::from(r"\\srv\share")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"C:\x")),
            PathBuf::from(r"C:\x")
        );
    }
}
