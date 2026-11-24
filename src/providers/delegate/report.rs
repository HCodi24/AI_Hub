use super::Delegation;
use crate::config::MutisConfig;
use crate::core::text::truncate_chars;

pub(super) const PRELOAD_MAX_LINES: usize = 150;

/// Current text of the small listed files, as `FILE:` blocks (the worker then skips those reads). Larger files, binary
/// files and files that contain a code fence stay for the worker to read by range.
pub(super) fn preload(folder: &str, files: &[String]) -> String {
    files.iter().filter_map(|file| {
        let text = std::fs::read_to_string(std::path::Path::new(folder).join(file.replace('\\', "/"))).ok()?;
        (text.lines().count() <= PRELOAD_MAX_LINES && !text.contains("```")).then(|| format!("FILE: {file}\n{}\n```\n", text.trim_end()))
    }).collect()
}

pub(super) fn worker_prompt(mutis: &MutisConfig, task: &Delegation, preloaded: &str, feedback: Option<&str>) -> String {
    let mut parts = Vec::new();
    if mutis.enabled && !mutis.worker_rules.trim().is_empty() {
        parts.push(mutis.worker_rules.clone());
    }
    // Applied at runtime, so existing configurations and sessions get the language rule too.
    parts.push(crate::core::rule::mutis_high_delegation::VIETNAMESE_RULE.to_owned());
    parts.push(format!("Task: {}", task.title.trim()));
    if !task.files.is_empty() {
        parts.push(format!("Files: {}", task.files.join(", ")));
    }
    parts.push(format!("Instructions:\n{}", task.instructions.trim()));
    if !task.acceptance.is_empty() {
        parts.push(format!("Acceptance criteria:\n{}", task.acceptance.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n")));
    }
    if !preloaded.is_empty() {
        parts.push(format!("Current content of the small listed files (already read for you; do not read them again):\n{preloaded}"));
    }
    if let Some(why) = feedback {
        parts.push(format!("The previous attempt failed: {why}. Do not investigate again: make the edit now, only in the listed files."));
    }
    parts.push("Make the edits yourself with the file tools; investigating without editing is a failed task. End with the files you changed and anything left undone.".into());
    parts.join("\n")
}

pub(in crate::providers) struct WorkerResult {
    pub(in crate::providers) summary: String,
    pub(in crate::providers) diff: String,
    pub(in crate::providers) error: Option<String>,
    /// Edits the scope refused (outside the listed files).
    pub(in crate::providers) refused: Vec<String>,
}

/// Why the app can tell, without the coordinator, that a worker run produced nothing usable. None = changed something.
pub(in crate::providers) fn auto_retry_reason(result: &WorkerResult) -> Option<String> {
    if !result.diff.trim().is_empty() {
        return None;
    }
    let mut why = result.error.as_deref().map_or("it changed no file".to_owned(), |e| format!("it failed with: {}", truncate_chars(e, 300)));
    if !result.refused.is_empty() {
        why.push_str(&format!("; edits refused as outside the listed files: {}", truncate_chars(&result.refused.join("; "), 300)));
    }
    Some(why)
}

/// The report the coordinator reads. The headings are Vietnamese (the app's own prose); the actor name, the task title,
/// the attempt numbers, the diff and the original error text stay verbatim.
pub(in crate::providers) fn report(task: &Delegation, who: &str, attempt: usize, max: usize, auto: usize, result: &WorkerResult) -> String {
    let auto_note = if auto > 0 {
        format!("Ứng dụng đã tự chạy lại worker {auto} lần trước báo cáo này vì một lần chạy không đổi file nào hoặc bị lỗi.\n")
    } else {
        String::new()
    };
    let changed = if result.diff.trim().is_empty() {
        "KHÔNG - worker không thay đổi file nào".to_owned()
    } else {
        format!("có, diff:\n{}", truncate_chars(&result.diff, 6000))
    };
    let status = result.error.as_deref().map_or("đã xong".to_owned(), |e| format!("LỖI: {e}"));
    format!(
        "Báo cáo của worker {who} cho \"{}\" (lần {attempt}/{max}, còn {} lần cho task này):\n{auto_note}Trạng thái: {status}\nCác file đã đổi: {changed}\nTóm tắt của worker:\n{}",
        task.title.trim(),
        max.saturating_sub(attempt),
        truncate_chars(&result.summary, 3000),
    )
}

/// Closes every report sent to the coordinator.
pub(super) const REVIEW_INSTRUCTION: &str = "Review every result above against your acceptance criteria and the diffs. If a task passes, say so and continue. If a task fails, answer with another mutis-delegate block with the SAME title and concrete corrections; the attempts left are shown in its report.";
