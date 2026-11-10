use crate::{chat::ChatStore, config::Config, mutis::EventLog, plans::PlanStore};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    sync::Arc,
    time::Instant,
};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: Arc<RwLock<Config>>,
    pub(crate) config_path: PathBuf,
    pub(crate) chats: ChatStore,
    pub(crate) plans: PlanStore,
    pub(crate) running: Arc<Mutex<HashMap<String, RunningChat>>>,
    pub(crate) extension_running: Arc<Mutex<HashMap<String, CancellationToken>>>,
    pub(crate) provider_status:
        Arc<Mutex<HashMap<String, (Instant, crate::api::providers::ConnectionTest)>>>,
    pub(crate) event_log: EventLog,
    pub(crate) dev_repo: Option<PathBuf>,
    pub(crate) dev_events: broadcast::Sender<String>,
    pub(crate) active_jobs: Arc<AtomicUsize>,
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
    ) -> Self {
        let plans = PlanStore::new(&config_path).expect("config path has a parent");
        let event_log = EventLog::new(&config_path).expect("config path has a parent");
        let (dev_events, _) = broadcast::channel(64);
        let state = Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            chats: ChatStore::new(chat_dir),
            plans,
            running: Arc::new(Mutex::new(HashMap::new())),
            extension_running: Arc::new(Mutex::new(HashMap::new())),
            provider_status: Arc::new(Mutex::new(HashMap::new())),
            event_log,
            dev_repo: dev_repo.clone(),
            dev_events,
            active_jobs: Arc::new(AtomicUsize::new(0)),
        };
        if let Some(repo) = dev_repo {
            crate::api::dev::spawn_dev_watcher(state.clone(), repo);
        }
        state
    }
}
