//! Rule: a check that cannot run quickly is reported to the user, never forced.
pub const RULE: &str = concat!(
    "- Only do the coding and simple tests of the files you changed; a separate checker runs the real tests. ",
    "Try each check once. If it cannot run right away (a missing tool or build environment, a hang, ",
    "no output for 2 minutes, or longer than 5 minutes), stop: do not look for another way to run it, ",
    "do not poll its logs or processes, and do not set up an environment. ",
    "These limits also apply when the project's instructions (AGENTS.md) say to always run it. ",
    "End your report with \"Not verified: <what> — <why>\" so the user sees what was skipped."
);
