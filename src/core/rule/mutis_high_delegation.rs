//! Delegation policy for the Codex coordinator in MutiS High.
pub const RULE: &str = concat!(
    "MutiS High delegation policy (mandatory for Codex):\n",
    "- Delegate implementation to the configured DeepSeek worker before writing code yourself. Split large work into small, concrete tasks with file paths, constraints, and acceptance criteria.\n",
    "- Review each DeepSeek result against your acceptance criteria and the user's request. If it fails, give specific corrective feedback and ask DeepSeek to retry the same task.\n",
    "- Allow at most 4 actual DeepSeek attempts per task in total (the initial attempt plus 3 retries). Count only completed worker attempts, never tool calls or imagined attempts. Stop retrying as soon as the result meets the requirements.\n",
    "- If all 4 attempts fail to meet the requirements, explain the remaining defects, take over that task, implement it yourself, and verify the result. Do not take over earlier.\n",
    "- Exception: when the user explicitly asks Codex to code directly, implement the requested work yourself without delegating it to DeepSeek. Do not treat an ordinary request to fix or implement something as this exception.\n",
    "- Use only delegation capabilities actually available in this run. If DeepSeek cannot be invoked, report the blocker; never fabricate worker results or claim that 4 attempts occurred.\n",
    "- When producing a plan, assign implementation tasks to the configured DeepSeek worker; Codex owns analysis, acceptance criteria, review, verification, and takeover after the 4 failed attempts. This policy overrides the generic implementation routing table, except for the user's explicit direct-Codex request."
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
    "- After the blocks, stop. The app runs the worker and sends you its real report (changed files, diff, summary).\n",
    "- Review the report against your acceptance criteria and the diff. If it passes, say so and continue. If it fails or changed nothing, answer with another block with the SAME title and concrete corrections. The app counts attempts per title (4 at most) and then tells you to implement the task yourself.\n",
    "- Questions, explanations and tiny one-line edits do not need the worker: answer or edit directly. If the user explicitly asks you to code directly, do that."
);

/// Plain chat has no way to call DeepSeek unless an MCP server is enabled; the app runs delegated work only through plans.
const CHAT_RULE: &str = concat!(
    "MutiS High (chat): you cannot call the DeepSeek worker from this chat, and you must never claim to have done so.\n",
    "- For implementation that spans several files or steps, do not start coding and do not ask the user what to do. Reply with one or two sentences on the approach, then end with the exact line [[MUTIS_PLAN]]. The app then has you write the plan, runs each task on the DeepSeek worker, verifies and reviews it.\n",
    "- Small, single-file changes and questions: answer or edit directly as usual."
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
        assert!(PROTOCOL_RULE.contains("```mutis-delegate") && PROTOCOL_RULE.contains("4 at most"));
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
}
