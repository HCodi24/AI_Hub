use super::*;

#[test]
fn worker_defaults_lists_only_the_worker_rules_in_order() {
    let defaults = worker_defaults();
    assert_eq!(
        defaults.lines().collect::<Vec<_>>(),
        [
            utf8::RULE,
            keep_features::RULE,
            report_unfinished::RULE,
            bounded_checks::RULE,
        ]
    );
    assert!(!defaults.contains(english_rules::RULE));
}

#[test]
fn first_request_rules_reports_a_synced_index() {
    let rules = first_request_rules(true, true);
    assert!(rules.contains("HAS a .codegraph folder"));
    assert!(rules.contains("just synced"));
}

#[test]
fn first_request_rules_reports_a_failed_sync() {
    assert!(first_request_rules(true, false).contains("could not sync"));
}

#[test]
fn first_request_rules_reports_a_missing_index_after_a_failed_build() {
    let rules = first_request_rules(false, false);
    assert!(rules.contains("DOES NOT HAVE"));
    assert!(rules.contains("could not; tell the user"));
}
