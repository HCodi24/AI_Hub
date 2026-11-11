//! Fixed rules for AI providers. One file per rule; each exposes `RULE`.
pub mod keep_features;
pub mod report_unfinished;
pub mod utf8;

/// Default text of `mutis.worker_rules`: every rule above, one per line.
pub fn worker_defaults() -> String {
    [utf8::RULE, keep_features::RULE, report_unfinished::RULE].join("\n")
}
