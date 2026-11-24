use super::{
    debater::Debater,
    turn::{run_turn, sse, TurnEnv},
};
use crate::{
    app::{AppState, JobGuard},
    config::MutisConfig,
    features::debate::{self, Debate, Round, Side, SIDES},
    mutis::Event,
};
use axum::response::sse::Event as SseEvent;
use serde_json::json;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(super) struct DebateRun {
    pub(super) state: AppState,
    pub(super) chat_id: Option<String>,
    pub(super) record: Debate,
    pub(super) debaters: [Debater; 2],
    pub(super) judge: Debater,
    pub(super) mutis: MutisConfig,
    pub(super) memory: String,
    pub(super) out: mpsc::Sender<SseEvent>,
    pub(super) cancel: CancellationToken,
}

pub(super) async fn run_debate(run: DebateRun) {
    let DebateRun {
        state,
        chat_id,
        mut record,
        mut debaters,
        judge,
        mutis,
        memory,
        out,
        cancel,
    } = run;
    let started = Instant::now();
    let _job = JobGuard::new(state.active_jobs.clone());
    let env = TurnEnv {
        folder: &record.folder,
        mutis: &mutis,
        out: &out,
        cancel: &cancel,
    };
    let _ = out.send(sse("debate", json!(record))).await;
    for index in 1..=record.max_rounds {
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
            let (mut turn, session) = run_turn(&debater, &env, prompt, side.key(), index).await;
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
            if broken || cancel.is_cancelled() {
                break;
            }
        }
        let converged = round.converged();
        let failed = SIDES.iter().any(|side| {
            let turn = round.turn(*side);
            turn.text.trim().is_empty() && turn.error.is_some()
        });
        record.rounds.push(round);
        let _ = state.debates.save(&record);
        if cancel.is_cancelled() {
            record.status = "stopped".into();
            persist(&state, chat_id.as_deref(), &mut record, started).await;
            let _ = out.send(sse("done", json!(record))).await;
            return;
        }
        if failed {
            record.status = "error".into();
            record.error = Some("Một bên không trả lời được — tranh luận dừng lại".into());
            persist(&state, chat_id.as_deref(), &mut record, started).await;
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
        &env,
        debate::judge_prompt(&record, Some(&memory)),
        "judge",
        0,
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
    if cancel.is_cancelled() {
        record.status = "stopped".into();
        record.error = Some("Tranh luận đã dừng".into());
    }
    persist(&state, chat_id.as_deref(), &mut record, started).await;
    let _ = state.event_log.append(&Event {
        ts: crate::core::time::now_secs(),
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
}

async fn persist(state: &AppState, chat_id: Option<&str>, record: &mut Debate, started: Instant) {
    record.elapsed_s = started.elapsed().as_secs();
    if let Some(id) = chat_id {
        let _ = record.save_chat(&state.chats, id, false);
    }
    let _ = state.debates.save(record);
}
