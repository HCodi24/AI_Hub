//! Fixed rules for AI providers. One file per rule; each exposes `RULE`.
pub mod codegraph;
pub mod bounded_checks;
pub mod chat_questions;
pub mod deepseek;
pub mod deepseek_flash;
pub mod english_rules;
pub mod keep_features;
pub mod mutis_checker;
pub mod mutis_high_delegation;
pub mod report_unfinished;
pub mod stay_in_folder;
pub mod utf8;

/// Default text of `mutis.worker_rules`: every rule above, one per line.
pub fn worker_defaults() -> String {
    [
        utf8::RULE,
        keep_features::RULE,
        report_unfinished::RULE,
        bounded_checks::RULE,
    ]
    .join("\n")
}

/// Rules attached to the first coding request of a session.
/// `indexed` = the app already checked whether the project has `.codegraph`, so the AI needs no command
/// (and no extra model call) to find out.
pub fn first_request_rules(indexed: bool, synced: bool) -> String {
    let status = if indexed { "HAS" } else { "DOES NOT HAVE" };
    let freshness = match (indexed, synced) {
        (true, true) => " The app just synced the index with the current code; do not ask to rebuild it.",
        (true, false) => " The app could not sync the index, so it may be out of date.",
        _ => " The app tried to build it and could not; tell the user.",
    };
    format!(
        "{}\nStatus checked by the MutiS app itself (not stated by the user; no need to run a command to check again): the project {status} a .codegraph folder.{freshness}\n{}",
        codegraph::RULE,
        stay_in_folder::RULE
    )
}
