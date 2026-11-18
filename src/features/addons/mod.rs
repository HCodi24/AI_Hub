//! List and import personal Claude add-ons, and install add-ons from GitHub into MutiS.

mod claude;
mod github;
mod paths;
mod skills;

pub(crate) use claude::personal_claude_home;
pub use claude::{
    copy_skill, overview, personal_claude_dir, personal_mcp_definition, remove_skill, Overview,
};
pub use github::{github_source, inspect_checkout, LinkFound};
pub use paths::valid_name;
pub(crate) use paths::{safe_component, valid_skill_name};
pub use skills::{install_checkout_skill, SkillInstall};
pub(crate) use skills::{
    load_skill, remove_shared_skill, shared_skill_catalogue, shared_skills_dir,
};

#[cfg(test)]
mod tests;
