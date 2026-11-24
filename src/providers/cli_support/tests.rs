use super::*;

mod cost_tests {
    use super::token_usage_log;
    use serde_json::json;

    #[test]
    fn costs_round_to_four_decimals_and_trim_trailing_zeros() {
        for (value, expected) in [
            (json!(0.5748474000000001), "$0.5748"),
            (json!("0.5748474000000001"), "$0.5748"),
            (json!(1.2300), "$1.23"),
            (json!("2.00000"), "$2"),
            (json!(0), "$0"),
            (json!(0.00001), "$0"),
            (json!("0.99999"), "$1"),
        ] {
            let usage = json!({"input_tokens": 12, "output_tokens": 3});
            let line = token_usage_log(&usage, Some(&value)).unwrap();
            assert_eq!(
                line,
                format!("Token: vào 12 (cache 0), ra 3, chi phí {expected}")
            );
        }
    }
}

mod claude_support_tests {
    #[test]
    fn provider_ids_require_at_most_80_safe_ascii_bytes() {
        assert!(!super::valid_provider_id(""));
        assert!(super::valid_provider_id(&"a".repeat(80)));
        assert!(!super::valid_provider_id(&"a".repeat(81)));
        assert!(!super::valid_provider_id("codex cli"));
        assert!(!super::valid_provider_id("codex.cli"));
        assert!(super::valid_provider_id("codex-cli_1"));
    }

    use super::{environment_names_to_remove, version_dir_key};

    #[test]
    fn removes_claude_and_anthropic_environment_names() {
        let names = [
            "CLAUDECODE",
            "CLAUDE_CODE_HOME",
            "ANTHROPIC_BASE_URL",
            "PATH",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        assert_eq!(environment_names_to_remove(&names), names[..3]);
    }

    #[test]
    fn sorts_claude_version_directories_numerically() {
        let older = version_dir_key("2.1.309").unwrap();
        let newer = version_dir_key("2.1.317").unwrap();
        assert!(newer > older);
        assert!(version_dir_key("not-a-version").is_none());
    }

    #[test]
    fn a_stopped_reply_is_marked_as_cut_short_in_the_replayed_history() {
        let messages: Vec<crate::chat::ChatMessage> = serde_json::from_value(serde_json::json!([
            {"role":"user","content":"write a story"},
            {"role":"assistant","content":"Once upon","error": crate::chat::STOPPED_NOTE},
            {"role":"user","content":"continue"}
        ]))
        .unwrap();
        let prompt = super::compose_prompt_for(&messages, false, "claude_cli");
        assert!(prompt.contains("Once upon [stopped before it finished]"));
        let with_files: Vec<crate::chat::ChatMessage> = serde_json::from_value(serde_json::json!([
            {"role":"user","content":"edit"},
            {"role":"assistant","content":"done","files":[{"path":"src/a.rs","added":1,"removed":0}]},
            {"role":"user","content":"next"}
        ]))
        .unwrap();
        assert!(super::compose_prompt_for(&with_files, false, "claude_cli").contains("[files it changed: src/a.rs]"));
    }

    #[test]
    fn parses_auth_status_account_without_credentials() {
        let account = super::parse_auth_account(
            br#"{"loggedIn":true,"email":"a@b.c","subscriptionType":"max","token":"secret"}"#,
        )
        .unwrap();
        assert!(account.logged_in);
        assert_eq!(account.email.as_deref(), Some("a@b.c"));
        assert_eq!(account.plan.as_deref(), Some("max"));
        let bare = super::parse_auth_account(br#"{"loggedIn":false,"email":""}"#).unwrap();
        assert!(!bare.logged_in && bare.email.is_none());
        assert!(super::parse_auth_account(b"{}").is_none());
    }

    #[test]
    fn auth_origin_marker_round_trips() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/auth-origin-test")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(super::auth_origin(&dir), None);
        super::set_auth_origin(&dir, Some("web"));
        assert_eq!(super::auth_origin(&dir).as_deref(), Some("web"));
        super::set_auth_origin(&dir, None);
        assert_eq!(super::auth_origin(&dir), None);
    }
}

mod prompt_tests {
    use super::*;
    use crate::chat::ChatMessage;

    fn message(role: &str, provider: &str, content: &str) -> ChatMessage {
        serde_json::from_value(serde_json::json!({"role": role, "content": content, "provider": provider})).unwrap()
    }

    #[test]
    fn a_fresh_session_gets_the_long_last_answer_and_who_wrote_it() {
        let long = "x".repeat(5_000);
        let chat = [message("user", "", "q1"), message("assistant", "codex_cli", &long), message("user", "", "what did you see?")];
        let prompt = compose_prompt_for(&chat, false, "claude_cli");
        assert!(prompt.contains("Assistant (Codex): ") && prompt.contains(&long));
        assert!(prompt.ends_with("Latest user message:\nwhat did you see?"));
    }

    #[test]
    fn a_resumed_session_only_hears_what_other_ais_said_meanwhile() {
        let chat = [
            message("user", "", "q1"),
            message("assistant", "claude_cli", "a1"),
            message("user", "", "q2"),
            message("assistant", "codex_cli", "codex answer"),
            message("user", "", "q3"),
        ];
        let prompt = compose_prompt_for(&chat, true, "claude_cli");
        assert!(prompt.contains("User: q2") && prompt.contains("Assistant (Codex): codex answer"));
        assert!(!prompt.contains("a1") && prompt.ends_with("q3"));
        // Nobody else spoke, or old chat without labels: just the latest message.
        assert_eq!(compose_prompt_for(&chat, true, "codex_cli"), "q3");
        assert_eq!(compose_prompt_for(&chat[..3], true, "claude_cli"), "q2");
        assert_eq!(compose_prompt_for(&chat, true, "deepseek"), "q3");
    }
}
