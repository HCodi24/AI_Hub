macro_rules! protocol_heading {
    () => {
        "MutiS delegation protocol (mandatory in this chat):\n"
    };
}

macro_rules! delegate_block_start {
    () => {
        concat!(
            "- Read the relevant files yourself first. Then end your reply with one fenced block per task:\n",
            "```mutis-delegate\n",
        )
    };
}

macro_rules! delegate_block_end {
    () => {
        "```\n"
    };
}

macro_rules! plan_all_tasks {
    () => {
        "- Plan ALL tasks of the job in this one reply, one block each, in order. Add \"depends_on\":[\"title of another task\"] only when a task needs that task's result: independent tasks start together up to the configured worker count when they list different files, while earlier ones are still being checked, and you hear back once when all are done.\n"
    };
}

macro_rules! after_delegate_blocks {
    () => {
        "- After the blocks, stop. The app runs the worker and sends you its real report (changed files, diff, summary), plus the project's own checks when they are enabled.\n"
    };
}

macro_rules! review_delegate_result {
    () => {
        concat!(
            "- Add \"trivial\":true to a block for a tiny low-risk change (a few lines of styling or text, a label, a rename, one field: nothing with logic, state, data or security, and nothing other code depends on). The app then skips the checker's check and you read the diff yourself. When unsure, leave it out.\n",
            "- Review the report against your acceptance criteria and the diff. If it passes, say so and continue. If it fails or changed nothing, answer with another block with the SAME title and concrete corrections.\n",
        )
    };
}

macro_rules! direct_work_rule {
    () => {
        "- Questions, explanations and tiny one-line edits do not need the worker: answer or edit directly. If the user explicitly asks you to code directly, do that.\n"
    };
}

macro_rules! vietnamese_rule_bullet {
    () => {
        concat!(
            "- Write every user-facing word in Vietnamese: your commentary, progress explanations, summaries, notes and review verdicts. ",
            "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
            "exactly as they are. This holds even when the user's request or the task instructions are in English; ",
            "these internal rules stay in English."
        )
    };
}
