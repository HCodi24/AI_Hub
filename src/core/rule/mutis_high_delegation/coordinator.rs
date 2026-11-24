/// Language rule shared by the worker prompt and the dev-chat preamble: the visible prose is Vietnamese,
/// the evidence stays verbatim.
pub const VIETNAMESE_RULE: &str = concat!(
    "Write every user-facing word in Vietnamese: commentary, progress explanations, summaries, notes and review verdicts. ",
    "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
    "exactly as they are. This holds even when the task instructions are in English."
);

pub const RULE: &str = concat!(
    "MutiS High delegation policy (mandatory for Codex):\n",
    "- Delegate implementation to the configured DeepSeek worker before writing code yourself. Split large work into small, concrete tasks with file paths, constraints, and acceptance criteria.\n",
    "- Review each DeepSeek result against your acceptance criteria and the user's request. If it fails, give specific corrective feedback and ask DeepSeek to retry the same task.\n",
    "- Allow at most 4 actual DeepSeek attempts per task in total (the initial attempt plus 3 retries). Count only completed worker attempts, never tool calls or imagined attempts. Stop retrying as soon as the result meets the requirements.\n",
    "- If all 4 attempts fail to meet the requirements, explain the remaining defects, take over that task, implement it yourself, and verify the result. Do not take over earlier.\n",
    "- Exception: when the user explicitly asks Codex to code directly, implement the requested work yourself without delegating it to DeepSeek. Do not treat an ordinary request to fix or implement something as this exception.\n",
    "- Use only delegation capabilities actually available in this run. If DeepSeek cannot be invoked, report the blocker; never fabricate worker results or claim that 4 attempts occurred.\n",
    "- When producing a plan, assign implementation tasks to the configured DeepSeek worker; Codex owns analysis, acceptance criteria, review, verification, and takeover after the 4 failed attempts. This policy overrides the generic implementation routing table, except for the user's explicit direct-Codex request.\n",
    vietnamese_rule_bullet!(),
);

/// Appended to the coordinator's rule while the "DeepSeek may do UI" switch is off.
pub const NO_UI_NOTE: &str = concat!(
    "UI layout and styling work is switched off for DeepSeek in this app.\n",
    "- Style and markup files (.css, .scss, .sass, .less, .html) are never run on DeepSeek: the app sends such a task back to you.\n",
    "- Component files (.tsx, .jsx, .vue, .svelte) are your call. A change to layout, spacing, sizes, styling or the structure of the markup: mark it \"hard\":true and do it yourself, deciding the exact values. Wiring data, props, state or text into a component is ordinary work: give it to DeepSeek."
);
