//! Rule: a project check that hangs is reported, never polled.
pub const RULE: &str = concat!(
    "- Run each long project check (e2e, dev server, full build) at most once, through the project's own script if it has one. ",
    "These limits also apply when the project's instructions (AGENTS.md) say to always run it: ",
    "if the check prints nothing new for 2 minutes or runs longer than 5 minutes, stop it, ",
    "read its log at most 3 times, do not poll processes or the clock, ",
    "and report it as \"not verified\" with the reason. Rely on the faster checks (lint, type check) instead."
);
