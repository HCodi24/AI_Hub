//! Early-stop detection shared by the Codex app-server adapter.
pub const EARLY_STOP_PHRASES: &[&str] = &[
    "incomplete",
    "not finished",
    "chưa xong",
    "chưa hoàn thành",
    "remaining work",
    "i stopped",
];

pub fn indicates_early_stop(message: &str) -> bool {
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let lowered = normalized
        .trim()
        .split("\n\n")
        .last()
        .unwrap_or_default()
        .to_lowercase();
    EARLY_STOP_PHRASES
        .iter()
        .any(|phrase| lowered.contains(phrase))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn early_stop_checks_only_the_last_paragraph() {
        assert!(indicates_early_stop(
            "Summary\n\nRemaining work: finish tests"
        ));
        assert!(!indicates_early_stop("Previously incomplete\n\nAll done."));
        assert!(indicates_early_stop("CHƯA HOÀN THÀNH"));
    }
}
