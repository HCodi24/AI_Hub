use crate::{app::AppState, providers::codex::transport::Client};
use serde_json::json;
use std::time::{Duration, Instant};

pub(crate) struct CodexConnection {
    pub(super) client: Client,
    pub(super) login_id: Option<String>,
    pub(super) url: Option<String>,
    pub(super) started: Instant,
}

pub(crate) async fn reap_connections(state: &AppState) {
    let mut sessions = state.codex_connections.lock().await;
    let expired = sessions
        .iter()
        .filter(|(_, s)| s.started.elapsed() >= Duration::from_secs(600))
        .map(|(id, _)| id.clone())
        .collect::<Vec<_>>();
    for id in expired {
        if let Some(mut session) = sessions.remove(&id) {
            session.client.close().await;
        }
    }
}

impl CodexConnection {
    pub(super) async fn cancel_and_close(mut self) {
    if let Some(login) = self.login_id.take() {
        let _ = self
            .client
            .call("account/login/cancel", json!({"loginId":login}))
            .await;
    }
    self.client.close().await;
    }
}

pub(in crate::api::providers) async fn close_connection(state: &AppState, id: &str) {
    let session = state.codex_connections.lock().await.remove(id);
    if let Some(session) = session {
        session.cancel_and_close().await;
    }
}
