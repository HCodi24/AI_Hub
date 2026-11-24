use super::*;

#[test]
fn prompt_carries_the_task_the_criteria_and_the_diff_and_asks_for_json_only() {
    let text = prompt(" T ", &["a.rs".into()], &["builds".into()], "done", "+line", &["npm test".into()], true, "");
    assert!(text.contains("Task: T") && text.contains("Files: a.rs") && text.contains("- builds") && text.contains("+line"));
    assert!(text.contains("ONLY this JSON") && text.contains("\"verdict\""));
    assert!(text.contains("you run the tests") && text.contains("- npm test"));
    assert!(prompt("T", &[], &[], "", "", &[], true, "").contains("none given") && prompt("T", &[], &[], "", "", &[], true, "").contains("Find the project's own"));
}

#[test]
fn prompt_asks_for_vietnamese_prose_but_keeps_the_json_contract() {
    let text = prompt("T", &["a.rs".into()], &["builds".into()], "done", "+line", &[], true, "");
    assert!(text.contains("Write the values of notes, problem and fix in Vietnamese"));
    assert!(text.contains("even when the task instructions are in English"));
    // The machine contract is untouched: keys, verdict tokens and severity values stay English.
    assert!(text.contains("\"verdict\":\"PASS\" or \"FAIL\"") && text.contains("\"severity\":\"high|medium|low\""));
    assert!(text.contains("Keep the JSON keys, the verdict PASS or FAIL and the severity high, medium or low exactly as shown"));
    assert!(text.contains("keep file paths, commands, model names and literal tool or error output verbatim"));
}

#[test]
fn own_rules_lead_the_prompt_and_the_app_switch_drops_the_test_instruction() {
    let own = prompt("T", &[], &[], "", "", &[], true, " no console.log ");
    assert!(own.starts_with("Rules from the user for this check:\nno console.log\n\nYou are an independent checker"));
    let off = prompt("T", &[], &[], "", "", &[], false, "");
    assert!(!off.contains("you run the tests") && off.contains("Reply with ONLY this JSON"));
}
