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
    #[serde(default)]
    chat_id: Option<String>,
    #[serde(default = "append_request_default")]
    append_request: bool,
    #[serde(default)]
    after_debate_id: Option<String>,
    folder: String,
    #[serde(default)]
    dev_chat: bool,
    #[serde(default)]
    provider_id: String,
    #[serde(default)]
    model: String,
}

fn append_request_default() -> bool {
    true
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
    if plan
        .chat_id
        .as_deref()
        .is_some_and(|id| !chat::valid_id(id))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
    // A linked plan cannot silently move to another chat.
    let mut newly_attached = plan.chat_id.is_some();
    if let Some(saved) = state
        .plans
        .get(&id)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    {
        if saved.chat_id.is_some() {
            newly_attached = false;
            plan.chat_id = saved.chat_id;
        }
    }
    plan.touch();
    state
        .plans
        .save(&plan)
        .map_err(|_| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Không lưu được kế hoạch"))?;
    plan.save_chat(&state.chats, false, false)
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    // Older linked debates had a chat, but their mini task list was not a chat message.
    if let Some(chat_id) = plan.chat_id.as_ref().filter(|_| newly_attached) {
        let reorder = || -> anyhow::Result<()> {
            for debate in state.debates.list()? {
                if debate.plan_id.as_deref() != Some(&plan.id)
                    || debate.chat_id.as_ref() != Some(chat_id)
                {
                    continue;
                }
                if let Some(mut record) = state.chats.get(chat_id)? {
                    let debate_index = record
                        .messages
                        .iter()
                        .position(|m| m.debate_id.as_deref() == Some(&debate.id));
                    let plan_index = record
                        .messages
                        .iter()
                        .position(|m| m.plan_id.as_deref() == Some(&plan.id));
                    if let (Some(after), Some(from)) = (debate_index, plan_index) {
                        if from > after + 1 {
                            let message = record.messages.remove(from);
                            record.messages.insert(after + 1, message);
                            chat::save_record(&state.chats, &mut record)?;
                        }
                    }
                }
            }
            Ok(())
        };
        reorder().map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(Json(plan))
}
pub(super) async fn generate_plan(
    State(state): State<AppState>,
    Json(input): Json<PlanRequest>,
) -> ApiResult<Response> {
    if input
        .chat_id
        .as_deref()
        .is_some_and(|id| !chat::valid_id(id))
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "chat_id không hợp lệ",
        ));
    }
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
    let memory = crate::features::memory::memory_context(
        &state.config_path,
        &folder.to_string_lossy(),
        &cfg.memory,
    );
    let request_context = format!("{memory}\n{request_context}");
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
            debate_id: None,
            plan_id: None,
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
    let mut pending = Plan::new(
        input.request.chars().take(80).collect(),
        folder_text.clone(),
        request_text.clone(),
        Vec::new(),
    )
    .with_dev_chat(input.dev_chat);
    pending.chat_id = input.chat_id.clone();
    pending.planning = true;
    pending
        .save_chat(&state.chats, input.append_request, true)
        .and_then(|()| state.plans.save(&pending))
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if let (Some(chat_id), Some(debate_id)) = (&pending.chat_id, &input.after_debate_id) {
        if let Ok(Some(mut record)) = state.chats.get(chat_id) {
            if let Some(index) = record
                .messages
                .iter()
                .position(|m| m.debate_id.as_ref() == Some(debate_id))
            {
                if let Some(plan_index) = record
                    .messages
                    .iter()
                    .position(|m| m.plan_id.as_ref() == Some(&pending.id))
                {
                    let message = record.messages.remove(plan_index);
                    record.messages.insert(index + 1, message);
                    chat::save_record(&state.chats, &mut record).map_err(|e| {
                        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
                    })?;
                }
            }
        }
    }
    tokio::spawn(async move {
        let _ = out_tx
            .send(chat::sse_event("planning", &json!(pending)))
            .await;
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
                _ = task_cancel.cancelled() => {
                    pending.planning = false;
                    pending.error = Some("Đã dừng lập kế hoạch".into());
                    let _ = task_state.plans.save(&pending);
                    let _ = pending.save_chat(&task_state.chats, false, false);
                    return;
                },
                _ = out_tx.closed() => { task_cancel.cancel(); continue; },
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
                        continue;
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
                                permission: None,
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
            Ok(mut plan) => {
                plan.id = pending.id.clone();
                plan.chat_id = pending.chat_id.clone();
                match task_state
                    .plans
                    .save(&plan)
                    .and_then(|()| plan.save_chat(&task_state.chats, false, false))
                {
                    Ok(()) => {
                        let _ = out_tx.send(chat::sse_event("plan", &json!(plan))).await;
                    }
                    Err(e) => {
                        let _ = out_tx
                            .send(chat::sse_event("error", &json!({"message":e.to_string()})))
                            .await;
                    }
                }
            }
            Err(e) => {
                pending.planning = false;
                pending.error = Some(e.clone());
                let _ = task_state.plans.save(&pending);
                let _ = pending.save_chat(&task_state.chats, false, false);
                let _ = out_tx
                    .send(chat::sse_event("error", &json!({"message":e})))
                    .await;
            }
        }
    });
    let stream_guard = CancelOnDrop(cancel.clone());
    let stream = async_stream::stream! {let _guard=stream_guard;let mut rx=ReceiverStream::new(out_rx);while let Some(event)=rx.next().await{yield Ok::<_,Infallible>(event)}};
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}
#[cfg(test)]
mod plan_tests {
    use super::*;
    use serde_json::Value;

    fn state() -> (AppState, PathBuf) {
        let root = std::env::temp_dir().join(format!("plan-chat-{}", uuid::Uuid::new_v4()));
        let state = AppState::new(
            crate::config::Config::seeded(),
            root.join("config.json"),
            root.join("chats"),
            None,
            0,
        );
        (state, root)
    }

    #[tokio::test]
    async fn post_rejects_invalid_chat_before_any_provider_call() {
        let (state, root) = state();
        let input = serde_json::from_value(
            json!({"request":"Chia việc", "folder":".", "chat_id":"../bad"}),
        )
        .unwrap();
        let result = generate_plan(State(state.clone()), Json(input)).await;
        assert_eq!(result.unwrap_err().status, StatusCode::BAD_REQUEST);
        assert!(state.chats.list().unwrap().is_empty());
        assert!(state.plans.list().unwrap().is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn plan_chat_append_roundtrip_and_put_updates_progress_summary() {
        let (state, root) = state();
        let input: PlanRequest = serde_json::from_value(json!({
            "request":"Làm giao diện", "folder":".", "chat_id": uuid::Uuid::new_v4().to_string()
        }))
        .unwrap();
        assert!(input.append_request);
        let mut plan = Plan::new(
            "Kế hoạch tiếng Việt".into(),
            input.folder,
            input.request,
            vec![],
        );
        plan.chat_id = input.chat_id;
        // Same append operation used by POST, without invoking a real planner.
        plan.save_chat(&state.chats, input.append_request, true)
            .unwrap();
        state.plans.save(&plan).unwrap();
        let id = plan.chat_id.clone().unwrap();
        let mut chat = state.chats.get(&id).unwrap().unwrap();
        assert_eq!(chat.messages.len(), 2);
        assert_eq!(chat.messages[0].content, "Làm giao diện");
        assert!(chat.messages[1].content.is_empty());
        assert_eq!(chat.messages[1].plan_id.as_deref(), Some(plan.id.as_str()));
        plan.dev_chat = true;
        plan.save_chat(&state.chats, false, true).unwrap();
        assert!(state.chats.get(&id).unwrap().unwrap().dev_chat);
        chat.messages
            .push(serde_json::from_value(json!({"role":"user", "content":"Tin khác"})).unwrap());
        state.chats.save(&chat).unwrap();
        plan.tasks = serde_json::from_value(json!([{
            "id":1, "title":"Sửa UI", "detail":"Giữ tiếng Việt", "assignee":"Codex", "kind":"code", "status":"chờ"
        }])).unwrap();
        let _ = save_plan(
            State(state.clone()),
            Path(plan.id.clone()),
            Json(plan.clone()),
        )
        .await
        .unwrap();
        let saved = state.chats.get(&id).unwrap().unwrap();
        assert!(saved.messages[1].content.contains("[chờ] Sửa UI · Codex"));
        plan.tasks[0].status = "xong".into();
        let _ = save_plan(
            State(state.clone()),
            Path(plan.id.clone()),
            Json(plan.clone()),
        )
        .await
        .unwrap();
        let saved = state.chats.get(&id).unwrap().unwrap();
        assert_eq!(saved.messages.len(), 3);
        assert_eq!(saved.messages[2].content, "Tin khác");
        assert!(saved.messages[1].content.contains("[xong]"));
        let roundtrip: ChatMessage = serde_json::from_value(json!(saved.messages[1])).unwrap();
        assert_eq!(roundtrip.plan_id.as_deref(), Some(plan.id.as_str()));
        let old: ChatMessage =
            serde_json::from_value(json!({"role":"assistant", "content":"cũ"})).unwrap();
        assert!(old.plan_id.is_none());
        assert!(json!(old).get("plan_id").is_none());
        assert_eq!(
            state.plans.get(&plan.id).unwrap().unwrap().chat_id,
            Some(id)
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn derived_plan_has_no_extra_user_message_and_summary_is_bounded() {
        let (state, root) = state();
        let mut plan = Plan::new("😀".repeat(1600), ".".into(), "Phán quyết".into(), vec![]);
        plan.chat_id = Some(uuid::Uuid::new_v4().to_string());
        plan.save_chat(&state.chats, false, true).unwrap();
        plan.save_chat(&state.chats, false, false).unwrap();
        let record = state
            .chats
            .get(plan.chat_id.as_deref().unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(record.messages.len(), 1);
        assert_eq!(record.messages[0].content.chars().count(), 1500);
        plan.chat_id = Some("invalid".into());
        assert!(plan.save_chat(&state.chats, false, false).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn adopting_old_linked_plan_places_card_after_debate_and_preserves_turns() {
        let (state, root) = state();
        let mut plan = Plan::new("Kế hoạch cũ".into(), ".".into(), "Yêu cầu".into(), vec![]);
        state.plans.save(&plan).unwrap();
        let chat_id = uuid::Uuid::new_v4().to_string();
        let debate = crate::features::debate::Debate {
            id: uuid::Uuid::new_v4().to_string(),
            title: "Tranh luận cũ".into(),
            chat_id: Some(chat_id.clone()),
            plan_id: Some(plan.id.clone()),
            ..Default::default()
        };
        state.debates.save(&debate).unwrap();
        debate.save_chat(&state.chats, &chat_id, true).unwrap();
        let mut chat = state.chats.get(&chat_id).unwrap().unwrap();
        chat.messages
            .push(serde_json::from_value(json!({"role":"user", "content":"Tin về sau"})).unwrap());
        state.chats.save(&chat).unwrap();
        plan.chat_id = Some(chat_id.clone());
        let _ = save_plan(
            State(state.clone()),
            Path(plan.id.clone()),
            Json(plan.clone()),
        )
        .await
        .unwrap();
        let saved = state.chats.get(&chat_id).unwrap().unwrap();
        assert_eq!(saved.messages.len(), 4);
        assert_eq!(saved.messages[1].debate_id, Some(debate.id));
        assert_eq!(saved.messages[2].plan_id, Some(plan.id));
        assert_eq!(saved.messages[3].content, "Tin về sau");
        std::fs::remove_dir_all(root).unwrap();
    }

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
