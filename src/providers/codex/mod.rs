//! App-owned Codex connection, configuration and runtime.
pub(crate) mod environment;
mod runtime;
pub(crate) mod transport;

pub(crate) use environment::{home, session_prefix};
pub use runtime::CodexProvider;
