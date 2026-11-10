//! HTTP API for "⚖ Tranh luận" (see features/debate.rs for the rules and prompts).

use crate::{
    api::{providers::cached_provider_test, ApiError, ApiResult},
    app::{AppState, JobGuard},
    chat::{self, ChatMessage},
    config::{Config, MutisConfig, MutisRole, ProviderConfig},
    features::debate::{self, Debate, Round, Side, Turn, SIDES},
    mutis::Event,
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tokio_util::sync::CancellationToken;

#[derive(Deserialize)]
pub(crate) struct DebateInput {
    request: String,
    folder: String,
    #[serde(default)]
    chat_id: Option<String>,
}

#[derive(Clone)]
struct Debater {
    provider: ProviderConfig,
    model: String,
    effort: chat::Effort,
    session: Option<String>,
}

fn parse_effort(value: &str) -> chat::Effort {
    serde_json::from_value(json!(value)).unwrap_or_default()
}

/// The connected CLI of `kind`, preferring the MutiS role's provider/model/effort when it is that kind.
async fn pick_debater(
    state: &AppState,
    cfg: &Config,
    kind: &str,
    role: &MutisRole,
) -> Option<Debater> {
    let mut candidates: Vec<&ProviderConfig> = cfg
        .providers
        .iter()
        .filter(|p| p.kind == kind && p.id == role.provider_id)
        .collect();
    candidates.extend(
        cfg.providers
            .iter()
            .filter(|p| p.kind == kind && p.id != role.provider_id),
    );
    for provider in candidates {
        let connected = cached_provider_test(state, &provider.id)
            .await
            .is_ok_and(|status| status.status == "connected");
        if !connected {
            continue;
        }
        let from_role = provider.id == role.provider_id;
        let model = if from_role && !role.model.is_empty() {
            role.model.clone()
        } else {
            provider.models.first().cloned().unwrap_or_default()
        };
        let effort = if from_role {
            parse_effort(&role.effort)
        } else {
            chat::Effort::Default
        };
        return Some(Debater {
            provider: provider.clone(),
            model,
            effort,
            session: None,
        });
    }
    None
}

fn sse(name: &str, data: serde_json::Value) -> SseEvent {
    SseEvent::default()
        .event(name)
        .json_data(data)
        .unwrap_or_default()
}

/// Run one prompt to completion on a debater, streaming its text/activity to the client tagged with `tag`
/// ("codex" | "claude" | "judge"). Returns the finished turn and the CLI session id (to continue next round).
#[allow(clippy::too_many_arguments)]
async fn run_turn(
    debater: &Debater,
    folder: &str,
    prompt: String,
    mutis: &MutisConfig,
    tag: &'static str,
    round: u32,
    out: &mpsc::Sender<SseEvent>,
    cancel: &CancellationToken,
) -> (Turn, Option<String>) {
    let started = std::time::Instant::now();
    let mut turn = Turn::default();
    let provider = match providers::for_kind(&debater.provider.kind) {
        Ok(provider) => provider,
        Err(error) => {
            turn.error = Some(error.to_string());
            return (turn, None);
        }
    };
    let request = ProviderRequest {
        provider: debater.provider.clone(),
        model: debater.model.clone(),
        folder: folder.to_owned(),
        // Debaters and the judge only read: nobody edits files during a debate.
        permission: chat::Permission::Read,
        effort: debater.effort,
        messages: vec![ChatMessage {
            debate_id: None,
            role: "user".into(),
            content: prompt,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            error: None,
        }],
        session_id: debater.session.clone(),
        mutis: mutis.clone(),
    };
    let (sender, mut receiver) = mpsc::channel::<ProviderEvent>(64);
    let run_cancel = cancel.clone();
    tokio::spawn(async move {
        if let Err(error) = provider.run(request, sender.clone(), run_cancel).await {
            let _ = sender
                .send(ProviderEvent::text("error", error.to_string()))
                .await;
        }
    });
    let mut text = String::new();
    let mut session = None;
    while let Some(event) = receiver.recv().await {
        let value = event.data["text"].as_str().unwrap_or_default().to_owned();
        match event.name {
            "delta" => {
                text.push_str(&value);
                let _ = out
                    .send(sse(
                        "delta",
                        json!({"side": tag, "round": round, "text": value}),
                    ))
                    .await;
            }
            "log" => {
                if let Some(tokens) = debate::tokens_from_log(&value) {
                    turn.tokens += tokens;
                }
                let _ = out
                    .send(sse(
                        "log",
                        json!({"side": tag, "round": round, "text": value}),
                    ))
                    .await;
            }
            "session" => session = event.data["session_id"].as_str().map(str::to_owned),
            "error" => turn.error = Some(value),
            _ => {}
        }
    }
    turn.elapsed_s = started.elapsed().as_secs();
    turn.text = text;
    (turn, session)
}

pub(crate) async fn start_debate(
    State(state): State<AppState>,
    Json(input): Json<DebateInput>,
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
    let request = input.request.trim().to_owned();
    if request.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Hãy nhập vấn đề cần tranh luận",
        ));
    }
    let folder = std::path::PathBuf::from(input.folder.trim());
    if !folder.is_dir() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Thư mục làm việc không tồn tại",
        ));
    }
    let folder = folder.to_string_lossy().into_owned();
    let cfg = state.config.read().await.clone();
    let memory = crate::features::memory::memory_context(&state.config_path, &folder, &cfg.memory);
    let codex = pick_debater(&state, &cfg, "codex_cli", &cfg.mutis.roles.big).await;
    let claude = pick_debater(&state, &cfg, "claude_cli", &cfg.mutis.roles.lead).await;
    let (Some(codex), Some(claude)) = (codex, claude) else {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tranh luận cần cả Codex CLI và Claude Code CLI đang kết nối (xem Cài đặt › Nguồn AI)",
        ));
    };
    // The judge is a SECOND, fresh Claude session (no shared context with the Claude debater).
    let judge = Debater {
        model: if cfg.mutis.debate_judge_model.trim().is_empty() {
            claude.model.clone()
        } else {
            cfg.mutis.debate_judge_model.trim().to_owned()
        },
        session: None,
        ..claude.clone()
    };
    let max_rounds = cfg.mutis.debate_rounds.clamp(1, 3);
    let mut record = Debate {
        chat_id: input.chat_id.clone(),
        id: uuid::Uuid::new_v4().to_string(),
        title: request
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(80)
            .collect(),
        request,
        folder,
        created_at: crate::mutis::now(),
        status: "running".into(),
        codex_provider: codex.provider.name.clone(),
        codex_model: codex.model.clone(),
        claude_provider: claude.provider.name.clone(),
        claude_model: claude.model.clone(),
        judge_model: judge.model.clone(),
        max_rounds,
        ..Default::default()
    };
    if let Some(id) = &input.chat_id {
        record.save_chat(&state.chats, id, true).map_err(|_| {
            ApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Không lưu được chat tranh luận",
            )
        })?;
    }
    let _ = state.debates.save(&record);

    let (out, receiver) = mpsc::channel::<SseEvent>(128);
    let cancel = CancellationToken::new();
    let task_cancel = cancel.clone();
    let task_state = state.clone();
    tokio::spawn(async move {
        let started = std::time::Instant::now();
        let _job = JobGuard::new(task_state.active_jobs.clone());
        let mutis = cfg.mutis.clone();
        let mut debaters = [codex, claude];
        let _ = out.send(sse("debate", json!(record))).await;
        for index in 1..=max_rounds {
            let _ = out.send(sse("round", json!({"round": index}))).await;
            let previous = record.rounds.last().cloned();
            // Claude speaks first; Codex then reads it and rebuts; next round Claude answers the rebuttal.
            let mut round = Round {
                index,
                ..Default::default()
            };
            for side in SIDES {
                let slot = if side == Side::Codex { 0 } else { 1 };
                let prompt = match (side, &previous) {
                    (Side::Claude, None) => {
                        debate::proposal_prompt(side, &record.request, Some(&memory))
                    }
                    (Side::Claude, Some(prev)) => debate::critique_prompt(
                        side,
                        &format!("{memory}\n{}", record.request),
                        &prev.claude.text,
                        &prev.codex.text,
                        index,
                    ),
                    (Side::Codex, _) => debate::critique_prompt(
                        side,
                        &format!("{memory}\n{}", record.request),
                        previous
                            .as_ref()
                            .map_or("", |prev| prev.codex.text.as_str()),
                        &round.claude.text,
                        index,
                    ),
                };
                let debater = debaters[slot].clone();
                let (mut turn, session) = run_turn(
                    &debater,
                    &record.folder,
                    prompt,
                    &mutis,
                    side.key(),
                    index,
                    &out,
                    &task_cancel,
                )
                .await;
                debaters[slot].session = session.or(debaters[slot].session.take());
                let (shown, agree, disagreements) = debate::split_stance(&turn.text);
                turn.text = shown;
                // Claude's round-1 turn is the opening proposal: no stance yet.
                if !(side == Side::Claude && index == 1) {
                    turn.agree = agree;
                    turn.disagreements = disagreements;
                }
                let _ = out
                    .send(sse(
                        "turn",
                        json!({"side": side.key(), "round": index, "turn": turn}),
                    ))
                    .await;
                let broken = turn.text.trim().is_empty() && turn.error.is_some();
                *round.turn_mut(side) = turn;
                if broken || task_cancel.is_cancelled() {
                    break;
                }
            }
            let converged = round.converged();
            let failed = SIDES.iter().any(|side| {
                let turn = round.turn(*side);
                turn.text.trim().is_empty() && turn.error.is_some()
            });
            record.rounds.push(round);
            let _ = task_state.debates.save(&record);
            if task_cancel.is_cancelled() {
                record.status = "stopped".into();
                record.elapsed_s = started.elapsed().as_secs();
                if let Some(id) = &input.chat_id {
                    let _ = record.save_chat(&task_state.chats, id, false);
                }
                let _ = task_state.debates.save(&record);
                let _ = out.send(sse("done", json!(record))).await;
                return;
            }
            if failed {
                record.status = "error".into();
                record.error = Some("Một bên không trả lời được — tranh luận dừng lại".into());
                record.elapsed_s = started.elapsed().as_secs();
                if let Some(id) = &input.chat_id {
                    let _ = record.save_chat(&task_state.chats, id, false);
                }
                let _ = task_state.debates.save(&record);
                let _ = out.send(sse("error", json!({"text": record.error}))).await;
                let _ = out.send(sse("done", json!(record))).await;
                return;
            }
            if converged {
                let _ = out
                    .send(sse(
                        "log",
                        json!({"side": "system", "text": "Hai bên đã thống nhất — dừng sớm"}),
                    ))
                    .await;
                break;
            }
        }
        let _ = out
            .send(sse("judging", json!({"model": judge.model})))
            .await;
        let (judge_turn, _) = run_turn(
            &judge,
            &record.folder,
            debate::judge_prompt(&record, Some(&memory)),
            &mutis,
            "judge",
            0,
            &out,
            &task_cancel,
        )
        .await;
        match debate::parse_verdict(&judge_turn.text) {
            Some(mut verdict) => {
                verdict.tokens = judge_turn.tokens;
                record.verdict = Some(verdict);
                record.status = "done".into();
            }
            None => {
                record.status = "error".into();
                record.error = Some(
                    judge_turn
                        .error
                        .unwrap_or_else(|| "Trọng tài trả lời không đúng định dạng JSON".into()),
                );
            }
        }
        if task_cancel.is_cancelled() {
            record.status = "stopped".into();
            record.error = Some("Tranh luận đã dừng".into());
        }
        record.elapsed_s = started.elapsed().as_secs();
        if let Some(id) = &input.chat_id {
            let _ = record.save_chat(&task_state.chats, id, false);
        }
        let _ = task_state.debates.save(&record);
        let _ = task_state.event_log.append(&Event {
            ts: crate::mutis::now(),
            plan_id: None,
            task_id: None,
            provider_id: "debate".into(),
            kind: "debate".into(),
            task_kind: None,
            ok: record.status == "done",
            duration_ms: None,
            tokens: Some(record.total_tokens()),
            detail: Some(record.title.clone()),
        });
        if let Some(verdict) = &record.verdict {
            let _ = out.send(sse("verdict", json!(verdict))).await;
        } else {
            let _ = out.send(sse("error", json!({"text": record.error}))).await;
        }
        let _ = out.send(sse("done", json!(record))).await;
    });

    let stream_cancel = cancel.clone();
    let stream = async_stream::stream! {
        // Closing the page stops the debate (kills both CLIs) like a normal chat.
        let _guard = CancelOnDrop(stream_cancel);
        let mut events = ReceiverStream::new(receiver);
        while let Some(event) = events.next().await {
            yield Ok::<_, Infallible>(event);
        }
    };
    Ok(Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

struct CancelOnDrop(CancellationToken);
impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

pub(crate) async fn list_debates(State(state): State<AppState>) -> ApiResult<Json<Vec<Debate>>> {
    state.debates.list().map(Json).map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Không đọc được lịch sử tranh luận",
        )
    })
}

pub(crate) async fn get_debate(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<Json<serde_json::Value>> {
    let debate = state
        .debates
        .get(&id)
        .map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "id tranh luận không hợp lệ"))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy tranh luận"))?;
    let plan_request = debate::plan_request(&debate);
    let mut value = json!(debate);
    value["plan_request"] = json!(plan_request);
    Ok(Json(value))
}

#[derive(Deserialize)]
pub(crate) struct PlanLink {
    plan_id: String,
}

pub(crate) async fn link_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<PlanLink>,
) -> ApiResult<Json<Debate>> {
    if !chat::valid_id(&id) || !chat::valid_id(&input.plan_id) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, "id không hợp lệ"));
    }
    let mut debate = state
        .debates
        .get(&id)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy tranh luận"))?;
    state
        .plans
        .get(&input.plan_id)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?
        .ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "Không tìm thấy kế hoạch"))?;
    if debate.status != "done" || debate.verdict.is_none() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "Tranh luận chưa có phán quyết hoàn tất",
        ));
    }
    debate.plan_id = Some(input.plan_id);
    state
        .debates
        .save(&debate)
        .map_err(|error| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;
    Ok(Json(debate))
}

#[cfg(test)]
mod plan_link_tests {
    use super::*;

    #[tokio::test]
    async fn link_roundtrip_and_invalid_ids() {
        let root = std::env::temp_dir().join(format!("debate-link-{}", uuid::Uuid::new_v4()));
        let state = AppState::new(
            Config::seeded(),
            root.join("config.json"),
            root.join("chats"),
            None,
            0,
        );
        let debate = Debate {
            id: uuid::Uuid::new_v4().to_string(),
            chat_id: Some(uuid::Uuid::new_v4().to_string()),
            status: "done".into(),
            verdict: Some(Default::default()),
            ..Default::default()
        };
        state.debates.save(&debate).unwrap();
        let plan = crate::plans::Plan::new("Kế hoạch".into(), "".into(), "".into(), vec![]);
        let link = |id: String, plan_id: String| {
            link_plan(State(state.clone()), Path(id), Json(PlanLink { plan_id }))
        };
        assert_eq!(
            link(debate.id.clone(), plan.id.clone())
                .await
                .unwrap_err()
                .status,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            link("bad".into(), plan.id.clone())
                .await
                .unwrap_err()
                .status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            link(debate.id.clone(), "../bad".into())
                .await
                .unwrap_err()
                .status,
            StatusCode::BAD_REQUEST
        );
        state.plans.save(&plan).unwrap();
        let _ = link(debate.id.clone(), plan.id.clone()).await.unwrap();
        let saved = state.debates.get(&debate.id).unwrap().unwrap();
        assert_eq!(saved.plan_id, Some(plan.id));
        assert_eq!(saved.chat_id, debate.chat_id);
        let mut old = json!(saved);
        old.as_object_mut().unwrap().remove("plan_id");
        old.as_object_mut().unwrap().remove("chat_id");
        let old: Debate = serde_json::from_value(old).unwrap();
        assert!(old.plan_id.is_none() && old.chat_id.is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
