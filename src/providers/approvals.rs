//! Commands a source wants to run that the chat's permission would refuse: the run waits here for the user's answer.
use std::{collections::HashMap, sync::{Mutex, OnceLock}};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

static PENDING: OnceLock<Mutex<HashMap<String, oneshot::Sender<bool>>>> = OnceLock::new();

fn pending() -> std::sync::MutexGuard<'static, HashMap<String, oneshot::Sender<bool>>> {
    PENDING.get_or_init(Default::default).lock().expect("approvals mutex poisoned")
}

/// How long a run waits for an answer (the user's setting; 120 s when the config cannot be read).
pub fn wait_seconds(config_path: &std::path::Path) -> u64 {
    std::fs::read(config_path).ok()
        .and_then(|bytes| serde_json::from_slice::<crate::config::Config>(&bytes).ok())
        .map_or(120, |config| u64::from(config.approval_wait_seconds))
}

/// Puts `shown` to the user as a card and waits for the answer for as long as their setting says; no answer in time counts as "no".
/// `None` = the run was stopped meanwhile.
pub async fn confirm(events: &mpsc::Sender<super::ProviderEvent>, cancel: &CancellationToken, shown: String) -> Option<bool> {
    let wait = crate::config::config_file_path().map_or(120, |path| wait_seconds(&path));
    let (id, answer) = ask();
    let _ = events.send(super::ProviderEvent { name: "permission_request", data: serde_json::json!({"id": id, "command": shown, "need": "write"}) }).await;
    let allowed = tokio::select! {
        _ = cancel.cancelled() => { forget(&id); return None; }
        result = tokio::time::timeout(std::time::Duration::from_secs(wait), answer) => matches!(result, Ok(Ok(true))),
    };
    forget(&id);
    Some(allowed)
}

/// A new question: its id goes to the chat, the receiver gets the answer.
pub fn ask() -> (String, oneshot::Receiver<bool>) {
    let id = uuid::Uuid::new_v4().to_string();
    let (sender, receiver) = oneshot::channel();
    pending().insert(id.clone(), sender);
    (id, receiver)
}

/// The user's answer; false when nobody is waiting for `id` any more (answered, timed out or stopped).
pub fn answer(id: &str, allow: bool) -> bool {
    pending().remove(id).is_some_and(|sender| sender.send(allow).is_ok())
}

pub fn forget(id: &str) {
    pending().remove(id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_answer_reaches_the_waiting_run_once() {
        let (id, receiver) = ask();
        assert!(answer(&id, true));
        assert_eq!(receiver.await, Ok(true));
        assert!(!answer(&id, false), "a second answer finds nobody waiting");
        let (id, _receiver) = ask();
        forget(&id);
        assert!(!answer(&id, true));
    }
}
