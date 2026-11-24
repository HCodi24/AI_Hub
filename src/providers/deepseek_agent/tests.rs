use super::history::{drop_unanswered_calls, trim_history, KEEP_HISTORY_CHARS};
use super::prompt::strip_dsml;
use super::usage::Usage;
use serde_json::json;

#[test]
fn a_stop_between_tool_calls_and_results_leaves_no_dangling_call() {
    let call = |id: &str| json!({"id":id,"function":{"name":"read_file"}});
    let mut cut = vec![json!({"role":"user","content":"a"}), json!({"role":"assistant","tool_calls":[call("1"), call("2")]}), json!({"role":"tool","tool_call_id":"1","content":"x"})];
    drop_unanswered_calls(&mut cut);
    assert_eq!(cut.len(), 1);
    let mut whole = vec![json!({"role":"user","content":"a"}), json!({"role":"assistant","tool_calls":[call("1")]}), json!({"role":"tool","tool_call_id":"1","content":"x"})];
    drop_unanswered_calls(&mut whole);
    assert_eq!(whole.len(), 3);
}

#[test]
fn history_is_trimmed_from_the_front_and_starts_at_a_user_message() {
    let small = vec![json!({"role":"user","content":"a"}), json!({"role":"assistant","content":"b"})];
    assert_eq!(trim_history(&small), small);
    let big = "x".repeat(KEEP_HISTORY_CHARS / 2);
    let long = vec![json!({"role":"user","content":"old"}), json!({"role":"assistant","content":big}), json!({"role":"tool","content":big}), json!({"role":"user","content":"new"}), json!({"role":"assistant","content":"ok"})];
    let kept = trim_history(&long);
    assert_eq!(kept[0]["content"], "new");
    assert_eq!(kept.len(), 2);
}

#[test]
fn leaked_tool_markup_is_not_an_answer() {
    assert_eq!(strip_dsml("Done.\n<｜DSML｜function_calls>x"), "Done.");
    assert_eq!(strip_dsml("<｜DSML｜function_calls>x"), "");
    assert_eq!(strip_dsml("  plain  "), "plain");
}

#[test]
fn token_usage_is_summed_and_logged_only_when_there_is_any() {
    let mut usage = Usage::default();
    assert_eq!(usage.line(), None);
    usage.add(&json!({"prompt_tokens":10,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":4}}));
    assert_eq!(usage.line(), Some("Token: vào 10 (cache 4), ra 5 (suy luận 0)".into()));
}
