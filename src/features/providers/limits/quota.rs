//! Usage-limit awareness: a source that said "you hit your limit" is not called again for a while, and its failure is
//! never retried, so a run does not burn attempts (or another source's quota) on calls that cannot succeed.
use std::{collections::HashMap, sync::{Mutex, OnceLock}, time::{Duration, Instant}};

/// shortcut: a fixed freeze instead of parsing the reset time out of the message; parse it when a source gives it in a stable form.
const FREEZE: Duration = Duration::from_secs(30 * 60);

fn frozen_map() -> &'static Mutex<HashMap<String, (Instant, String)>> {
    static MAP: OnceLock<Mutex<HashMap<String, (Instant, String)>>> = OnceLock::new();
    MAP.get_or_init(Mutex::default)
}

/// Whether an error text says the source ran out of its usage allowance (not a fault the retry could fix).
pub fn is_limit(error: &str) -> bool {
    let text = error.to_lowercase();
    ["usage limit", "hit your limit", "rate limit", "quota"].iter().any(|needle| text.contains(needle))
}

pub fn freeze(source: &str, why: &str) {
    frozen_map().lock().unwrap_or_else(|e| e.into_inner()).insert(source.to_owned(), (Instant::now() + FREEZE, why.to_owned()));
}

pub fn thaw(source: &str) {
    frozen_map().lock().unwrap_or_else(|e| e.into_inner()).remove(source);
}

/// The error the source gave, while it is still frozen.
pub fn frozen(source: &str) -> Option<String> {
    let mut map = frozen_map().lock().unwrap_or_else(|e| e.into_inner());
    match map.get(source) {
        Some((until, why)) if *until > Instant::now() => Some(why.clone()),
        Some(_) => { map.remove(source); None }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_errors_are_told_apart_and_frozen_per_source() {
        assert!(is_limit("Codex báo lỗi: You’ve hit your usage limit. Upgrade to Pro"));
        assert!(is_limit("429 Rate limit exceeded"));
        assert!(!is_limit("it changed no file"));
        assert_eq!(frozen("q-a"), None);
        freeze("q-a", "limit until tomorrow");
        assert_eq!(frozen("q-a").as_deref(), Some("limit until tomorrow"));
        assert_eq!(frozen("q-b"), None, "another source is untouched");
    }
}
