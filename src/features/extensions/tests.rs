use super::catalog::CATALOG;
use super::*;

#[tokio::test]
async fn first_request_rules_say_whether_the_index_is_current() {
    let rules = crate::core::rule::first_request_rules(true, false);
    assert!(rules.contains("may be out of date"));
    assert!(crate::core::rule::first_request_rules(true, true).contains("just synced"));
    assert!(crate::core::rule::first_request_rules(false, false).contains("could not"));
}

#[test]
fn first_request_rules_keep_the_ai_inside_the_project_folder() {
    for indexed in [true, false] {
        assert!(crate::core::rule::first_request_rules(indexed, indexed).contains(crate::core::rule::stay_in_folder::RULE));
    }
    assert!(!crate::core::rule::worker_defaults().contains(crate::core::rule::stay_in_folder::RULE));
}

#[tokio::test]
async fn prepare_never_fails_for_a_missing_folder() {
    let out = prepare(std::path::Path::new("Z:/does/not/exist")).await;
    assert!(!out.indexed && out.notes.is_empty());
}

#[test]
fn catalog_actions_are_fixed_and_resolvable() {
    let (definition, install) = action("codegraph", "install").expect("catalog install action");
    assert_eq!(definition.executable, "codegraph");
    assert_eq!(install.args, ["i", "-g", "@colbymchenry/codegraph"]);
    assert_eq!(install.label, "Cài đặt");
    assert!(action("codegraph", "arbitrary-command").is_none());
}

#[test]
fn catalog_strings_do_not_contain_mojibake() {
    for definition in CATALOG {
        assert!(!definition.name.contains('\u{00c3}'));
        assert!(!definition.description.contains('\u{00c3}'));
        for action in definition.actions {
            assert!(!action.label.contains('\u{00c3}'));
        }
    }
}
