use crate::{chat::ChatStore, config::Config, mutis::EventLog, plans::PlanStore};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    process::{Child, ChildStdin},
    sync::{broadcast, mpsc, Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub(crate) port: u16,
    pub(crate) config: Arc<RwLock<Config>>,
    pub(crate) config_path: PathBuf,
    pub(crate) chats: ChatStore,
    pub(crate) plans: PlanStore,
    pub(crate) running: Arc<Mutex<HashMap<String, RunningChat>>>,
    pub(crate) extension_running: Arc<Mutex<HashMap<String, CancellationToken>>>,
    pub(crate) provider_status:
        Arc<Mutex<HashMap<String, (Instant, crate::api::providers::ConnectionTest)>>>,
    pub(crate) claude_login: Arc<Mutex<Option<ClaudeLoginSession>>>,
    pub(crate) event_log: EventLog,
    pub(crate) dev_repo: Option<PathBuf>,
    pub(crate) dev_events: broadcast::Sender<String>,
    pub(crate) active_jobs: Arc<AtomicUsize>,
}

pub(crate) struct ClaudeLoginSession {
    pub(crate) id: String,
    pub(crate) provider_id: String,
    pub(crate) child: Child,
    pub(crate) stdin: ChildStdin,
    pub(crate) stdout: mpsc::Receiver<String>,
    pub(crate) output_tail: Vec<String>,
    pub(crate) stderr_tail: Arc<Mutex<Vec<String>>>,
    pub(crate) last_activity: Instant,
}

#[derive(Clone)]
pub(crate) struct RunningChat {
    pub(crate) request_id: Uuid,
    pub(crate) cancel: CancellationToken,
}

pub(crate) struct JobGuard(pub(crate) Arc<AtomicUsize>);
impl JobGuard {
    pub(crate) fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::SeqCst);
        Self(counter)
    }
}
impl Drop for JobGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

impl AppState {
    pub fn new(
        config: Config,
        config_path: PathBuf,
        chat_dir: PathBuf,
        dev_repo: Option<PathBuf>,
        port: u16,
    ) -> Self {
        let plans = PlanStore::new(&config_path).expect("config path has a parent");
        let event_log = EventLog::new(&config_path).expect("config path has a parent");
        let (dev_events, _) = broadcast::channel(64);
        let state = Self {
            port,
            config: Arc::new(RwLock::new(config)),
            config_path,
            chats: ChatStore::new(chat_dir),
            plans,
            running: Arc::new(Mutex::new(HashMap::new())),
            extension_running: Arc::new(Mutex::new(HashMap::new())),
            provider_status: Arc::new(Mutex::new(HashMap::new())),
            claude_login: Arc::new(Mutex::new(None)),
            event_log,
            dev_repo: dev_repo.clone(),
            dev_events,
            active_jobs: Arc::new(AtomicUsize::new(0)),
        };
        spawn_claude_login_reaper(state.clone());
        if let Some(repo) = dev_repo {
            crate::api::dev::spawn_dev_watcher(state.clone(), repo);
        }
        state
    }
}

fn spawn_claude_login_reaper(state: AppState) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            let mut session = state.claude_login.lock().await;
            if session
                .as_ref()
                .is_some_and(|login| login.last_activity.elapsed() >= Duration::from_secs(600))
            {
                if let Some(mut expired) = session.take() {
                    let _ = expired.child.kill().await;
                }
            }
        }
    });
}
