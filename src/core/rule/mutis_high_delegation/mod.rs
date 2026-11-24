//! Delegation policy for the Codex coordinator in MutiS High.

/// The coordinator always analyses and splits the job first, whatever model the worker is; only how fine the split is differs.
macro_rules! split_rule {
    () => {
        "- Always analyse the job and split it before delegating, whatever the worker is. A task is one concern with a clear boundary: a few files at most and one result you can check in the diff. Never put a whole feature, or several layers (backend, web, adapters), into one task: give each its own block and link them with depends_on. A task that is merely wide is split into several tasks, whoever does it.\n"
    };
}

#[macro_use]
mod shared;

mod coordinator;
mod deepseek;
mod other;

pub use coordinator::{NO_UI_NOTE, RULE, VIETNAMESE_RULE};

use deepseek::PROTOCOL_RULE;
use other::OTHER_PROTOCOL_RULE;

#[cfg(test)]
mod tests;

pub fn for_coordinator(enabled: bool, level: &str, provider_kind: &str) -> &'static str {
    if enabled && level == "high" && matches!(provider_kind, "codex_cli" | "claude_cli") {
        RULE
    } else {
        ""
    }
}

/// Which model runs the delegated tasks: DeepSeek (exact-edit protocol) or any other model (goal-and-limits protocol).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Worker {
    DeepSeek,
    Other,
}

/// Rule for a plain chat turn: the delegation policy only when a way to reach the worker exists (the app's own
/// delegation, or an MCP server). DeepSeek gets the exact-edit protocol, any other worker the lighter one.
/// Otherwise Codex works directly and only keeps the language rule.
pub fn for_chat(enabled: bool, level: &str, provider_kind: &str, has_mcp: bool, worker: Option<Worker>) -> &'static str {
    match for_coordinator(enabled, level, provider_kind) {
        "" => "",
        _ if worker == Some(Worker::DeepSeek) => PROTOCOL_RULE,
        _ if worker == Some(Worker::Other) => OTHER_PROTOCOL_RULE,
        _ if has_mcp => RULE,
        _ => VIETNAMESE_RULE,
    }
}
