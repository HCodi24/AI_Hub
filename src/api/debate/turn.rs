use super::debater::Debater;
use crate::{
    chat::{self, ChatMessage},
    config::MutisConfig,
    features::debate::{self, Turn},
    providers::{self, ProviderEvent, ProviderRequest},
};
use axum::response::sse::Event as SseEvent;
use serde_json::json;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(super) fn sse(name: &str, data: serde_json::Value) -> SseEvent {
    SseEvent::default()
        .event(name)
        .json_data(data)
        .unwrap_or_default()
}

pub(super) struct TurnEnv<'a> {
    pub(super) folder: &'a str,
    pub(super) mutis: &'a MutisConfig,
    pub(super) out: &'a mpsc::Sender<SseEvent>,
    pub(super) cancel: &'a CancellationToken,
}

/// Run one prompt to completion on a debater, streaming its text/activity to the client tagged with `tag`
/// ("codex" | "claude" | "judge"). Returns the finished turn and the CLI session id (to continue next round).
pub(super) async fn run_turn(
    debater: &Debater,
    env: &TurnEnv<'_>,
    prompt: String,
    tag: &'static str,
    round: u32,
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
        folder: env.folder.to_owned(),
        // Debaters and the judge only read: nobody edits files during a debate.
        permission: chat::Permission::Read,
        effort: debater.effort,
        messages: vec![ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "user".into(),
            content: prompt,
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: String::new(),
            model: String::new(),
            error: None,
        }],
        session_id: debater.session.clone(),
        mutis: env.mutis.clone(),
        mcp: Vec::new(),
        write_scope: Vec::new(),
        checker_session_id: None,
    };
    let (sender, mut receiver) = mpsc::channel::<ProviderEvent>(64);
    let run_cancel = env.cancel.clone();
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
                let _ = env.out
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
                let _ = env.out
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
