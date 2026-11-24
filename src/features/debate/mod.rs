//! "⚖ Tranh luận": Codex and Claude debate a problem (read-only), then a SECOND, fresh Claude session judges.
//!
//! Round 1: both propose independently. Round 2+: each critiques the other's proposal and revises its own,
//! ending with a stance block. The debate stops early when both agree. The judge never took part in the debate,
//! so it is not biased towards its own proposal.

mod parse;
mod prompts;
mod store;
mod types;

pub use parse::{parse_verdict, split_stance, tokens_from_log};
pub use prompts::{critique_prompt, judge_prompt, plan_request, proposal_prompt};
pub use store::DebateStore;
pub use types::{Debate, Round, Side, Turn, SIDES};

#[cfg(test)]
mod tests;
