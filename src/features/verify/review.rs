#[cfg(test)]
pub(super) fn may_create_fix(verdict: &str, has_blocker: bool, rounds_used: u32, rounds_max: u32) -> bool {
    verdict == "FAIL" && has_blocker && rounds_used < rounds_max
}

#[cfg(test)]
pub(super) struct ReviewTaskState {
    pub(super) verify_ok: bool,
}
#[derive(Debug, PartialEq, Eq)]
#[cfg(test)]
pub(super) enum ReviewAction {
    Skip,
    Complete,
    Fail,
    CreateFix,
}
#[cfg(test)]
pub(super) fn next_review_action(
    task: &ReviewTaskState,
    verdict: &str,
    blocking_issue: bool,
    rounds_used: u32,
    max_rounds: u32,
) -> ReviewAction {
    next_action(
        task.verify_ok,
        verdict,
        blocking_issue,
        rounds_used,
        max_rounds,
    )
}
#[cfg(test)]
pub(super) fn next_action(
    verify_ok: bool,
    verdict: &str,
    blocking_issue: bool,
    rounds_used: u32,
    max_rounds: u32,
) -> ReviewAction {
    if verdict == "SKIPPED" && !verify_ok {
        ReviewAction::Fail
    } else if verdict == "SKIPPED" {
        ReviewAction::Skip
    } else if verdict != "FAIL" {
        ReviewAction::Complete
    } else if blocking_issue && rounds_used < max_rounds {
        ReviewAction::CreateFix
    } else {
        ReviewAction::Fail
    }
}
