//! The independent checker of MutiS High (an optional read-only Codex): what it is asked, and what the coordinator is told about it.

/// Appended to the coordinator's rule when the checker is on.
pub const NOTE: &str = concat!(
    "An independent read-only checker reviews every worker result in parallel, and its verdict is part of the report you receive.\n",
    "- Judge each of its issues yourself: if one is valid, answer with a mutis-delegate block with the SAME title and the correction; if it is wrong, say why and do not send it to the worker.\n",
    "- High and medium issues were already sent back to the worker by the app (as a fix of the same task, counted in its 4 attempts); low ones only reach you. You decide the outcome."
);

/// The prompt of one check. The working tree may already hold later tasks' edits, so the diff is the evidence.
pub fn prompt(title: &str, files: &[String], acceptance: &[String], summary: &str, diff: &str) -> String {
    let criteria = if acceptance.is_empty() { "- (none given: judge by the title)".to_owned() } else { acceptance.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n") };
    format!(
        concat!(
            "You are an independent, read-only checker. Judge ONE finished task against its acceptance criteria and the diff below.\n",
            "The working tree may already contain edits of later tasks: base your verdict on the diff; read files only for context. You cannot and must not edit anything.\n\n",
            "Task: {title}\nFiles: {files}\nAcceptance criteria:\n{criteria}\n\nWorker summary:\n{summary}\n\nDiff of this task:\n{diff}\n\n",
            "Reply with ONLY this JSON: {{\"verdict\":\"PASS\" or \"FAIL\",\"issues\":[{{\"severity\":\"high|medium|low\",\"file\":\"\",\"problem\":\"\",\"fix\":\"\"}}],\"notes\":\"\"}}. ",
            "Use high or medium only for a defect that makes the task fail its acceptance criteria; use low for style or optional improvements."
        ),
        title = title.trim(), files = files.join(", "), criteria = criteria,
        summary = crate::core::text::truncate_chars(summary, 2000), diff = crate::core::text::truncate_chars(diff, 12000),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_the_task_the_criteria_and_the_diff_and_asks_for_json_only() {
        let text = prompt(" T ", &["a.rs".into()], &["builds".into()], "done", "+line");
        assert!(text.contains("Task: T") && text.contains("Files: a.rs") && text.contains("- builds") && text.contains("+line"));
        assert!(text.contains("ONLY this JSON") && text.contains("\"verdict\""));
        assert!(prompt("T", &[], &[], "", "").contains("none given"));
    }
}
