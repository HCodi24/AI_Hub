//! The independent checker of MutiS High (an optional read-only Codex): what it is asked, and what the coordinator is told about it.

/// Appended to the coordinator's rule when the checker is on.
pub const NOTE: &str = concat!(
    "An independent read-only checker reviews every worker result in parallel, and its verdict is part of the report you receive.\n",
    "- Judge each of its issues yourself: if one is valid, answer with a mutis-delegate block with the SAME title and the correction; if it is wrong, say why and do not send it to the worker.\n",
    "- High and medium issues were already sent back to the worker by the app (as a fix of the same task, counted in its 4 attempts); low ones only reach you. You decide the outcome."
);

/// The prompt of one check. The working tree may already hold later tasks' edits, so the diff is the evidence.
pub fn prompt(title: &str, files: &[String], acceptance: &[String], summary: &str, diff: &str, commands: &[String], app_rules: bool, own_rules: &str) -> String {
    let criteria = if acceptance.is_empty() { "- (none given: judge by the title)".to_owned() } else { acceptance.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n") };
    // The worker cannot run commands, so running the tests is this checker's job.
    let testing = if !app_rules {
        String::new()
    } else if commands.is_empty() {
        "The worker cannot run commands: you run the tests. Find the project's own test, type-check or build commands (package.json scripts, scripts/, Cargo.toml) and run the ones that cover this diff.".to_owned()
    } else {
        format!("The worker cannot run commands: you run the tests. Run the project's check command(s) from the project folder:\n{}", commands.iter().map(|c| format!("- {c}")).collect::<Vec<_>>().join("\n"))
    };
    let own = if own_rules.trim().is_empty() { String::new() } else { format!("Rules from the user for this check:\n{}\n\n", own_rules.trim()) };
    let prompt = format!(
        concat!(
            "You are an independent checker. Judge ONE finished task against its acceptance criteria, the diff below and the real test results.\n",
            "The working tree may already contain edits of later tasks: base your verdict on the diff; read files only for context. You cannot and must not edit any file: run only commands that check (tests, type checks, linters).\n",
            "{testing}\n",
            "A failing check that this diff causes is a high issue. If a command cannot run (missing tool, no permission), say so in notes: that alone is not a FAIL.\n\n",
            "Task: {title}\nFiles: {files}\nAcceptance criteria:\n{criteria}\n\nWorker summary:\n{summary}\n\nDiff of this task:\n{diff}\n\n",
            "Reply with ONLY this JSON: {{\"verdict\":\"PASS\" or \"FAIL\",\"issues\":[{{\"severity\":\"high|medium|low\",\"file\":\"\",\"problem\":\"\",\"fix\":\"\"}}],\"notes\":\"\"}}. ",
            "Use high or medium only for a defect that makes the task fail its acceptance criteria; use low for style or optional improvements.\n",
            "Write the values of notes, problem and fix in Vietnamese, even when the task instructions are in English. ",
            "Keep the JSON keys, the verdict PASS or FAIL and the severity high, medium or low exactly as shown; ",
            "keep file paths, commands, model names and literal tool or error output verbatim."
        ),
        testing = testing, title = title.trim(), files = files.join(", "), criteria = criteria,
        summary = crate::core::text::truncate_chars(summary, 2000), diff = crate::core::text::truncate_chars(diff, 12000),
    );
    format!("{own}{prompt}")
}

#[cfg(test)]
mod tests;
