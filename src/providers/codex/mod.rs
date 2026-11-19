//! App-owned Codex connection, configuration and runtime.
pub(crate) mod compact;
pub(crate) mod environment;
pub(crate) mod gate;
mod runtime;
pub(crate) mod transport;

pub(crate) use environment::{home, session_prefix};
pub use runtime::CodexProvider;
