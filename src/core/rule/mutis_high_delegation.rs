//! Delegation policy for the Codex coordinator in MutiS High.

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
    "- Write every user-facing word in Vietnamese: your commentary, progress explanations, summaries, notes and review verdicts. ",
    "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
    "exactly as they are. This holds even when the user's request or the task instructions are in English; ",
    "these internal rules stay in English."
);

pub fn for_coordinator(enabled: bool, level: &str, provider_kind: &str) -> &'static str {
    if enabled && level == "high" && provider_kind == "codex_cli" {
        RULE
    } else {
        ""
    }
}

/// Plain chat with a DeepSeek worker the app can run: Codex hands work over in a fenced block and the app answers with the real report.
const PROTOCOL_RULE: &str = concat!(
    "MutiS High delegation protocol (mandatory for Codex in this chat):\n",
    "- You coordinate. Implementation goes to the DeepSeek worker; you analyse, write acceptance criteria, review and verify. Do not edit project files yourself while delegating.\n",
    "- Read the relevant files yourself first. Then end your reply with one fenced block per task:\n",
    "```mutis-delegate\n",
    "{\"title\":\"short unique task name\",\"files\":[\"exact/path.ext\"],\"instructions\":\"what to change, with names, line ranges and expected behaviour\",\"acceptance\":[\"a condition you can check in the diff\"]}\n",
    "```\n",
    "- The worker has no context beyond the block: name exact files (and symbols or line ranges) and put everything it needs in the instructions. Never create a separate investigation task for the worker.\n",
    "- Plan ALL tasks of the job in this one reply, one block each, in order. Add \"depends_on\":[\"title of another task\"] only when a task needs that task's result: independent tasks start at once, one after another, while earlier ones are still being checked, and you hear back once when all are done.\n",
    "- The worker cannot run commands: never put running tests, builds or type checks in its instructions or acceptance. The independent checker (Luna) runs them and its verdict is part of the report; when there is no checker you run them yourself after the report.\n",
    "- After the blocks, stop. The app runs the worker and sends you its real report (changed files, diff, summary), plus the project's own checks when they are enabled.\n",
    "- Add \"hard\":true to a block when the task needs real reasoning (logic, algorithms, layout decisions, an unknown cause): the app then gives it to a stronger second checker model (Luna 2) instead of DeepSeek. DeepSeek is for mechanical edits whose exact change you can describe.\n",
    "- Add \"trivial\":true to a block for a tiny low-risk change (a few lines of styling or text, a label, a rename, one field: nothing with logic, state, data or security, and nothing other code depends on). The app then skips Luna's check and you read the diff yourself. When unsure, leave it out.
",
    "- Review the report against your acceptance criteria and the diff. If it passes, say so and continue. If it fails or changed nothing, answer with another block with the SAME title and concrete corrections.\n",
    "- The app counts attempts per title and climbs by itself when a checker exists: DeepSeek twice (Luna checks), then Luna fixes it and Luna 2 checks; a hard task gets Luna 2 twice (Luna checks). When those are spent it tells you to implement the task yourself and verify it (the user then judges the result). Without a checker DeepSeek gets 4 attempts.\n",
    "- Questions, explanations and tiny one-line edits do not need the worker: answer or edit directly. If the user explicitly asks you to code directly, do that.\n",
    "- Write every user-facing word in Vietnamese: your commentary, progress explanations, summaries, notes and review verdicts. ",
    "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
    "exactly as they are. This holds even when the user's request or the task instructions are in English; ",
    "these internal rules stay in English."
);

/// Appended to the coordinator's rule while the "DeepSeek may do UI" switch is off.
pub const NO_UI_NOTE: &str = concat!(
    "UI layout and styling work is switched off for DeepSeek in this app.\n",
    "- Style and markup files (.css, .scss, .sass, .less, .html) are never run on DeepSeek: the app hands such a task to Luna 2 when it can, otherwise it sends it back to you.\n",
    "- Component files (.tsx, .jsx, .vue, .svelte) are your call. A change to layout, spacing, sizes, styling or the structure of the markup: mark it \"hard\":true or do it yourself, and decide the exact values. Wiring data, props, state or text into a component is ordinary work: give it to DeepSeek."
);

/// Plain chat has no way to call DeepSeek unless an MCP server is enabled; the app runs delegated work only through plans.
const CHAT_RULE: &str = concat!(
    "MutiS High (chat): you cannot call the DeepSeek worker from this chat, and you must never claim to have done so.\n",
    "- For implementation that spans several files or steps, do not start coding and do not ask the user what to do. Reply with one or two sentences on the approach, then end with the exact line [[MUTIS_PLAN]]. The app then has you write the plan, runs each task on the DeepSeek worker, verifies and reviews it.\n",
    "- Small, single-file changes and questions: answer or edit directly as usual.\n",
    "- Write every user-facing word in Vietnamese: your commentary, progress explanations, summaries, notes and review verdicts. ",
    "Keep code, paths, commands, model names, task titles, JSON keys, protocol enum values and literal tool or error output ",
    "exactly as they are. This holds even when the user's request or the task instructions are in English; ",
    "these internal rules stay in English."
);

/// Rule for a plain chat turn: the full delegation policy only when a tool to reach DeepSeek exists.
pub fn for_chat(enabled: bool, level: &str, provider_kind: &str, has_mcp: bool, has_worker: bool) -> &'static str {
    match for_coordinator(enabled, level, provider_kind) {
        "" => "",
        _ if has_worker => PROTOCOL_RULE,
        _ if has_mcp => RULE,
        _ => CHAT_RULE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_rule_depends_on_how_deepseek_can_be_reached() {
        assert_eq!(for_chat(true, "high", "codex_cli", true, false), RULE);
        assert_eq!(for_chat(true, "high", "codex_cli", false, false), CHAT_RULE);
        assert_eq!(for_chat(true, "high", "codex_cli", false, true), PROTOCOL_RULE);
        assert_eq!(for_chat(true, "max", "codex_cli", false, true), "");
        assert!(PROTOCOL_RULE.contains("```mutis-delegate") && PROTOCOL_RULE.contains("\"hard\":true") && PROTOCOL_RULE.contains("Luna 2") && PROTOCOL_RULE.contains("\"trivial\":true"));
        assert!(PROTOCOL_RULE.lines().count() >= 10, "escapes must stay \\n, not literal newlines inside strings");
    }

    #[test]
    fn delegation_is_scoped_to_enabled_high_codex() {
        assert_eq!(for_coordinator(true, "high", "codex_cli"), RULE);
        assert_eq!(for_coordinator(false, "high", "codex_cli"), "");
        assert_eq!(for_coordinator(true, "max", "codex_cli"), "");
        assert_eq!(for_coordinator(true, "high", "openai_compat"), "");
        assert_eq!(for_coordinator(true, "high", "claude_cli"), "");
    }

    #[test]
    fn every_coordinator_rule_requires_vietnamese_visible_prose() {
        for rule in [RULE, PROTOCOL_RULE, CHAT_RULE] {
            assert!(rule.contains("user-facing word in Vietnamese"), "missing language rule: {rule}");
            assert!(rule.contains("even when the user's request or the task instructions are in English"));
            // The evidence the coordinator must keep verbatim is named, so the rule cannot be read as "translate everything".
            assert!(rule.contains("paths") && rule.contains("JSON keys") && rule.contains("literal tool or error output"));
        }
        // The shared constant is the same instruction the worker and the dev chat receive.
        assert!(VIETNAMESE_RULE.contains("Vietnamese") && VIETNAMESE_RULE.contains("exactly as they are"));
    }
}
