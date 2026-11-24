mod provider_tests {
    #[test]
    fn turn_cost_is_the_growth_of_the_sessions_running_total() {
        assert_eq!(super::super::parser::turn_cost(Some("c-new"), Some(2.0), false), Some(2.0), "a new session: the total is the turn");
        assert_eq!(super::super::parser::turn_cost(Some("c-new"), Some(2.5), true).map(|c| (c * 100.0).round()), Some(50.0));
        assert_eq!(super::super::parser::turn_cost(Some("c-old"), Some(8.0), true), None, "resumed with no baseline: unknown, not the whole total");
        assert_eq!(super::super::parser::turn_cost(Some("c-old"), Some(8.4), true).map(|c| (c * 100.0).round()), Some(40.0));
    }

    #[test]
    fn session_size_is_the_last_model_call_not_the_turn_total() {
        let usage = serde_json::json!({"input_tokens":99,"iterations":[
            {"input_tokens":2,"cache_read_input_tokens":1000,"cache_creation_input_tokens":10},
            {"input_tokens":3,"cache_read_input_tokens":2000,"cache_creation_input_tokens":20}]});
        assert_eq!(super::super::parser::context_size(&usage), Some(2023));
        assert_eq!(super::super::parser::context_size(&serde_json::json!({"input_tokens":5})), None, "no per-call data: the size is unknown, not guessed");
    }

    use super::super::parser::{parse_claude_line, ClaudeParser};
    use crate::chat::Permission;

    #[test]
    fn permission_arguments_only_allow_commands_for_full() {
        for (permission, mode) in [
            (Permission::Read, "manual"),
            (Permission::Write, "acceptEdits"),
            (Permission::Full, "acceptEdits"),
        ] {
            let args = super::super::permission::permission_args(permission);
            let args: Vec<_> = args.iter().map(|arg| arg.to_str().unwrap()).collect();
            assert_eq!(&args[..2], &["--permission-mode", mode]);
            if permission == Permission::Full {
                assert_eq!(&args[2..], &["--allowedTools", "Bash", "PowerShell"]);
            } else {
                assert_eq!(args.len(), 2);
            }
            assert!(!args.iter().any(|arg| arg.contains("bypassPermissions")));
        }
    }

    #[test]
    fn claude_asks_before_a_tool_the_mode_does_not_cover() {
        use crate::providers::cli_support::Reaction;
        let request = |tool: &str| serde_json::json!({"type":"control_request","request_id":"r1","request":{"subtype":"can_use_tool","tool_name":tool,"input":{"command":"echo hi > a.txt"}}}).to_string();
        let ask = super::super::permission::interaction(Permission::Read);
        assert!(ask(r#"{"type":"assistant"}"#).is_none());
        match ask(&request("Bash")) {
            Some(Reaction::Ask { shown, need, reply }) => {
                assert!(shown.contains("echo hi"));
                assert_eq!(need, "write");
                let yes: serde_json::Value = serde_json::from_str(&reply(true)).unwrap();
                assert_eq!(yes["response"]["request_id"], "r1");
                assert_eq!(yes["response"]["response"]["behavior"], "allow");
                assert_eq!(yes["response"]["response"]["updatedInput"]["command"], "echo hi > a.txt");
                assert!(reply(false).contains("deny"));
            }
            _ => panic!("a shell command must be put to the user"),
        }
        assert!(matches!(ask(&request("Read")), Some(Reaction::Reply(text)) if text.contains("allow")));
        assert!(matches!(ask(&request("AskUserQuestion")), Some(Reaction::Reply(text)) if text.contains("deny")));
        assert!(matches!(super::super::permission::interaction(Permission::Write)(&request("Bash")), Some(Reaction::Ask { need: "full", .. })));
    }

    #[test]
    fn plan_mode_writing_its_plan_offers_one_level_up_only_in_read() {
        let line = serde_json::json!({"type":"assistant","message":{"content":[{
            "type":"tool_use","name":"Write","input":{"file_path":"C:\\data\\plans\\x.md","content":"plan"}}]}}).to_string();
        for (permission, expected) in [(Permission::Read, true), (Permission::Write, false)] {
            let mut parser = ClaudeParser { permission, ..Default::default() };
            let denied: Vec<_> = parser.parse(&line).into_iter().filter(|e| e.name == "permission_denied").collect();
            assert_eq!(denied.len(), usize::from(expected));
            if expected { assert_eq!(denied[0].data["need"], "write"); }
            assert!(parser.parse(&line).iter().all(|e| e.name != "permission_denied"));
        }
        let said = r#"{"type":"result","subtype":"success","result":"Plan mode is active, so I haven't created c.txt."}"#;
        for (permission, expected) in [(Permission::Read, 1), (Permission::Write, 0)] {
            let events = ClaudeParser { permission, ..Default::default() }.parse(said);
            assert_eq!(events.iter().filter(|e| e.name == "permission_denied").count(), expected);
        }
        let other = serde_json::json!({"type":"assistant","message":{"content":[{
            "type":"tool_use","name":"Write","input":{"file_path":"src/a.rs","content":"x"}}]}}).to_string();
        assert!(ClaudeParser { permission: Permission::Read, ..Default::default() }.parse(&other).iter().all(|e| e.name != "permission_denied"));
    }

    #[test]
    fn command_denial_hint_is_emitted_once_per_run_except_full() {
        for permission in [Permission::Read, Permission::Write, Permission::Full] {
            for content in [
                serde_json::json!("Bash requires approval"),
                serde_json::json!([{"type": "text", "text": "You haven't granted PowerShell"}]),
            ] {
                let mut parser = ClaudeParser {
                    permission,
                    ..Default::default()
                };
                let line = serde_json::json!({"type":"user","message":{"content":[{
                    "type":"tool_result", "is_error":true, "content":content
                }]}})
                .to_string();
                let first = parser.parse(&line);
                assert_eq!(first.len(), 2 * usize::from(permission != Permission::Full));
                if permission != Permission::Full {
                    // Offers exactly one level up: Read -> write, Write -> full.
                    assert_eq!(first[0].name, "permission_denied");
                    assert_eq!(first[0].data["need"], if permission == Permission::Read { "write" } else { "full" });
                    assert_eq!(first[1].name, "log");
                    assert_eq!(first[1].data["text"],
                        "Lệnh bị chặn: chọn Quyền › Toàn quyền (chạy lệnh) để cho phép AI chạy lệnh.");
                }
                assert!(parser.parse(&line).is_empty());
            }
        }
        let mut parser = ClaudeParser::default();
        for (kind, error, content) in [
            ("tool_result", false, "requires approval"),
            ("text", true, "requires approval"),
            ("tool_result", true, "file missing"),
        ] {
            let line = serde_json::json!({"type":"user","message":{"content":[{
                "type":kind,"is_error":error,"content":content
            }]}})
            .to_string();
            assert!(parser.parse(&line).is_empty());
        }
    }

    #[test]
    fn maps_claude_result_subtypes_and_returns_success_text() {
        let max_turns =
            parse_claude_line(r#"{"type":"result","subtype":"error_max_turns","result":"limit"}"#);
        assert_eq!(
            max_turns[0].data["text"],
            "Claude dừng vì hết số lượt (max turns): limit"
        );
        let other = parse_claude_line(
            r#"{"type":"result","subtype":"error_max_budget_usd","result":"budget"}"#,
        );
        assert!(other[0].data["text"]
            .as_str()
            .unwrap()
            .contains("error_max_budget_usd"));
        let success =
            parse_claude_line(r#"{"type":"result","subtype":"success","result":"answer"}"#);
        assert_eq!(success[0].name, "delta");
    }
    #[test]
    fn partial_text_suppresses_full_assistant_text_and_result_duplicate() {
        let mut parser = ClaudeParser::default();
        let partial = parser.parse(r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"partial"}}}"#);
        assert_eq!(partial.len(), 1);
        let complete = parser.parse(
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"partial"}]}}"#,
        );
        assert!(complete.is_empty());
        let result = parser.parse(r#"{"type":"result","subtype":"success","result":"partial"}"#);
        assert!(result.is_empty());
    }
    #[test]
    fn detects_missing_result_event_with_default_state() {
        assert!(super::super::missing_result_event(false, false));
        assert!(!super::super::missing_result_event(true, false));
        assert!(!super::super::missing_result_event(false, true));
        let mut parser = ClaudeParser::default();
        parser.parse(r#"{"type":"result","subtype":"success","result":"ok"}"#);
        assert!(parser.result_seen);
    }
}

mod separator_tests {
    use super::super::parser::ClaudeParser;

    fn deltas(events: Vec<crate::providers::ProviderEvent>) -> String {
        events
            .iter()
            .filter(|event| event.name == "delta")
            .map(|event| event.data["text"].as_str().unwrap_or_default())
            .collect()
    }

    #[test]
    fn text_from_separate_assistant_messages_is_not_glued_together() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        // First assistant message (streamed), then a tool call, then a second message with more text.
        for line in [
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Câu đầu."}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Câu đầu."}]}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Câu sau."}}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "Câu đầu.\n\nCâu sau.");
    }

    #[test]
    fn a_message_without_text_adds_no_blank_gap() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"A"}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"A"},{"type":"tool_use","name":"Read","input":{}}]}}"#,
            r#"{"type":"stream_event","event":{"type":"message_start"}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{}}]}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "A");
    }

    #[test]
    fn a_skill_run_shows_as_its_own_line() {
        let mut parser = ClaudeParser::default();
        let mut shown = String::new();
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","delta":{"type":"text_delta","text":"Trước"}}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Trước"},{"type":"tool_use","name":"Skill","input":{"skill":"ponytail"}}]}}"#,
        ] {
            shown.push_str(&deltas(parser.parse(line)));
        }
        assert_eq!(shown, "Trước\n\n✦ ponytail\n\n");
    }
}
