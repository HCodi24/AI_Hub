//! Rule for the in-app DeepSeek agent only (added to its system prompt, never sent to other providers).
pub const RULE: &str = concat!(
    "DeepSeek rules:\n",
    "- Use only the tools given to you in this run. You have no MCP and no CodeGraph; never ask to build or read .codegraph.\n",
    "- Read a file before editing it; do not guess file contents or paths.\n",
    "- Do not repeat a tool call that already returned the answer."
);
