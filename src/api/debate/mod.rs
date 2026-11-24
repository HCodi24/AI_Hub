//! HTTP API for "⚖ Tranh luận" (see features/debate.rs for the rules and prompts).

mod debater;
mod history;
mod run;
mod start;
mod turn;
#[cfg(test)]
mod tests;

pub(crate) use history::{attach_chat, get_debate, list_debates};
pub(crate) use start::start_debate;
