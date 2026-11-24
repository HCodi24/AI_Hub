//! Delegation policy for the MutiS lead.

macro_rules! split_rule {
    () => {
        "- Always analyse the job and split it before delegating, whatever the worker is. A task is one concern with a clear boundary: a few files at most and one result you can check in the diff. Never put a whole feature, or several layers (backend, web, adapters), into one task: give each its own block and link them with depends_on. A task that is merely wide is split into several tasks, whoever does it.\n"
    };
}

#[macro_use]
mod shared;
mod protocol;

#[cfg(test)]
mod tests;

/// Visible prose is Vietnamese; the evidence stays verbatim.
pub const VIETNAMESE_RULE: &str = concat!(
    "Write every user-facing word in Vietnamese: commentary, progress explanations, summaries, notes and review verdicts. ",
    "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
    "exactly as they are. This holds even when the task instructions are in English."
);

fn for_coordinator(can_delegate: bool) -> &'static str {
    if can_delegate {
        protocol::RULE
    } else {
        VIETNAMESE_RULE
    }
}

pub fn for_chat(is_mutis: bool, has_mcp: bool, has_worker: bool) -> &'static str {
    if is_mutis {
        for_coordinator(has_mcp || has_worker)
    } else {
        ""
    }
}
