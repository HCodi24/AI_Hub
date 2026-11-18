//! List and import personal Claude add-ons, and install add-ons from GitHub into MutiS.

mod claude;
mod github;
mod paths;
mod skills;

pub use claude::{
    copy_skill, overview, personal_claude_dir, personal_mcp_definition, remove_skill, Overview,
};
pub use github::{github_source, inspect_checkout, LinkFound};
pub(crate) use paths::safe_component;
pub use paths::valid_name;
pub use skills::{install_checkout_skill, managed_skill_context, SkillInstall};

#[cfg(test)]
mod tests;
