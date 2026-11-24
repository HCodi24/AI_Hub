use super::{LimitWindow, Limits};
use crate::core::fs::write_atomic;
use crate::core::time::now_secs;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

pub(super) fn cache() -> &'static Mutex<HashMap<String, Limits>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Limits>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(load_cache()))
}

fn cache_path() -> Option<PathBuf> {
    Some(
        crate::config::config_file_path()
            .ok()?
            .parent()?
            .join("limits-cache.json"),
    )
}

fn load_cache() -> HashMap<String, Limits> {
    cache_path()
        .and_then(|path| fs::read(path).ok())
        .and_then(|contents| serde_json::from_slice(&contents).ok())
        .unwrap_or_default()
}

pub(super) fn persist_cache(entries: &HashMap<String, Limits>) {
    let Some(path) = cache_path() else {
        return;
    };
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let Ok(contents) = serde_json::to_vec_pretty(entries) else {
        return;
    };
    // Cache persistence is best-effort; a failed rename leaves the existing cache intact.
    let _ = write_atomic(&path, &contents, false);
}

pub(super) fn save(id: &str, windows: Vec<LimitWindow>) -> Limits {
    let limits = Limits {
        windows,
        updated_at: Some(now_secs()),
    };
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(id.to_owned(), limits.clone());
    limits
}
