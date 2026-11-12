//! Fixed rules for AI providers. One file per rule; each exposes `RULE`.
pub mod codegraph;
pub mod keep_features;
pub mod report_unfinished;
pub mod utf8;

/// Default text of `mutis.worker_rules`: every rule above, one per line.
pub fn worker_defaults() -> String {
    [utf8::RULE, keep_features::RULE, report_unfinished::RULE].join("\n")
}

/// Rules attached to the first coding request of a session.
/// `indexed` = the app already checked whether the project has `.codegraph`, so the AI needs no command
/// (and no extra model call) to find out.
pub fn first_request_rules(indexed: bool) -> String {
    let status = if indexed { "ĐÃ CÓ" } else { "CHƯA CÓ" };
    format!(
        "{}\nTrạng thái (app đã kiểm tra sẵn, không cần chạy lệnh để kiểm tra lại): dự án {status} thư mục .codegraph.",
        codegraph::RULE
    )
}
