use super::*;

mod file_tools_tests {
    use super::*;
    use crate::config::ProviderConfig;

    #[test]
    fn full_permission_roundtrips_and_write_stays_default() {
        let full: Permission = serde_json::from_str("\"full\"").unwrap();
        assert_eq!(full, Permission::Full);
        assert_eq!(serde_json::to_string(&full).unwrap(), "\"full\"");
        assert_eq!(Permission::default(), Permission::Write);
    }

    fn source(kind: &str, agent_capable: bool) -> ProviderConfig {
        ProviderConfig {
            id: "p".into(),
            kind: kind.into(),
            agent_capable,
            ..Default::default()
        }
    }

    #[test]
    fn mutis_file_tools_follow_the_leads_kind_and_the_permission() {
        assert_eq!(file_tools_for(&source("codex_cli", false), Permission::Write), FileTools::None);
        assert_eq!(file_tools_for(&source("openai_compat", false), Permission::Write), FileTools::None);
        assert_eq!(file_tools_for(&source("openai_compat", true), Permission::Read), FileTools::Read);
        assert_eq!(file_tools_for(&source("anthropic", false), Permission::Full), FileTools::Write);
    }

    #[test]
    fn no_file_tools_is_a_normal_chat_for_any_source() {
        assert_eq!(
            agent_permission(
                &source("anthropic", false),
                FileTools::None,
                Permission::Write
            ),
            Ok(None)
        );
        assert_eq!(
            agent_permission(
                &source("openai_compat", true),
                FileTools::None,
                Permission::Write
            ),
            Ok(None)
        );
    }

    #[test]
    fn file_tools_map_to_agent_read_or_write() {
        let deploy = source("openai_compat", true);
        assert_eq!(
            agent_permission(&deploy, FileTools::Read, Permission::Write),
            Ok(Some(Permission::Read))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Write, Permission::Write),
            Ok(Some(Permission::Write))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Write, Permission::Full),
            Ok(Some(Permission::Full))
        );
        assert_eq!(
            agent_permission(&deploy, FileTools::Read, Permission::Full),
            Ok(Some(Permission::Read))
        );
    }

    #[test]
    fn anthropic_api_is_agent_capable_and_receives_full_permission() {
        let anthropic = source("anthropic", false);
        assert_eq!(
            agent_permission(&anthropic, FileTools::Write, Permission::Full),
            Ok(Some(Permission::Full))
        );
        assert_eq!(
            agent_permission(&anthropic, FileTools::Read, Permission::Write),
            Ok(Some(Permission::Read))
        );
    }

    #[test]
    fn file_tools_are_refused_when_the_source_is_not_agent_capable() {
        assert!(agent_permission(
            &source("openai_compat", false),
            FileTools::Write,
            Permission::Write
        )
        .is_err());
        assert!(agent_permission(
            &source("codex_cli", true),
            FileTools::Read,
            Permission::Write
        )
        .is_err());
    }

    #[test]
    fn file_tools_values_deserialize_from_lowercase_and_default_to_none() {
        let tools: FileTools = serde_json::from_str("\"read\"").unwrap();
        assert_eq!(tools, FileTools::Read);
        assert_eq!(FileTools::default(), FileTools::None);
    }
}

mod plan_task_chat_tests {
    use super::*;

    #[test]
    fn plan_task_chats_are_hidden_from_the_history_list() {
        let directory = std::env::temp_dir().join(format!("ai-hub-chat-{}", uuid::Uuid::new_v4()));
        let store = ChatStore::new(directory.clone());
        let request = |plan: Option<&str>, task: Option<&str>| -> ChatRequest {
            serde_json::from_value(serde_json::json!({
                "chat_id": uuid::Uuid::new_v4().to_string(),
                "provider_id": "p",
                "model": "m",
                "folder": "C:/x",
                "messages": [{"role": "user", "content": "hi"}],
                "plan_id": plan,
                "task_id": task,
            }))
            .unwrap()
        };
        let normal = new_record(&request(None, None));
        let task = new_record(&request(Some("plan-1"), Some("t1")));
        let plan_only = new_record(&request(Some("plan-1"), None));
        assert_eq!(task.plan_task_of.as_deref(), Some("plan-1"));
        assert!(plan_only.plan_task_of.is_none());
        for chat in [&normal, &task, &plan_only] {
            store.save(chat).unwrap();
        }
        let ids: Vec<String> = store.list().unwrap().into_iter().map(|c| c.id).collect();
        assert!(ids.contains(&normal.id) && ids.contains(&plan_only.id));
        assert!(!ids.contains(&task.id));
        let _ = std::fs::remove_dir_all(directory);
    }
}

mod message_model_tests {
    use super::*;

    #[test]
    fn old_message_json_without_model_loads_with_an_empty_model() {
        let message: ChatMessage =
            serde_json::from_str(r#"{"role":"assistant","content":"old reply"}"#).unwrap();
        assert_eq!(message.model, "");
        assert_eq!(message.content, "old reply");
    }

    #[test]
    fn old_chat_record_json_without_model_loads_with_empty_models() {
        let record: ChatRecord = serde_json::from_value(serde_json::json!({
            "id": "c", "title": "t", "provider_id": "p", "model": "m", "folder": "f",
            "updated_at": 0,
            "messages": [
                {"role": "user", "content": "hi"},
                {"role": "assistant", "content": "hello"}
            ]
        }))
        .unwrap();
        assert_eq!(record.messages[0].model, "");
        assert_eq!(record.messages[1].model, "");
    }

    #[test]
    fn assistant_placeholder_requires_absent_tasks() {
        let mut message = ChatMessage::assistant("codex_cli", "test-model".into());
        assert_eq!(message.provider, "codex_cli");
        assert_eq!(message.model, "test-model");
        assert!(message.is_empty_placeholder());

        message.tasks = Some(Vec::new());
        assert!(!message.is_empty_placeholder());
    }

    #[test]
    fn empty_model_is_not_written_but_a_real_model_roundtrips() {
        let unknown = ChatMessage {
            debate_id: None,
            plan_id: None,
            role: "assistant".into(),
            content: "hi".into(),
            attachments: Vec::new(),
            reasoning: String::new(),
            logs: Vec::new(),
            diff: String::new(),
            files: Vec::new(),
            tasks: None,
            provider: String::new(),
            model: String::new(),
            error: None,
        };
        let json = serde_json::to_value(&unknown).unwrap();
        assert!(json.get("model").is_none(), "unknown model stays out of the JSON");
        let known = ChatMessage {
            model: "claude-sonnet-4".into(),
            ..unknown
        };
        let json = serde_json::to_value(&known).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4");
        let back: ChatMessage = serde_json::from_value(json).unwrap();
        assert_eq!(back.model, "claude-sonnet-4");
    }
}

mod switch_source_tests {
    use super::*;

    fn record() -> ChatRecord {
        serde_json::from_value(serde_json::json!({
            "id": "c", "title": "t", "provider_id": "claude-cli", "model": "m", "folder": "f",
            "messages": [], "session_id": "claude-session", "updated_at": 0
        }))
        .unwrap()
    }

    #[test]
    fn switching_away_and_back_resumes_each_sources_own_session() {
        let mut chat = record();
        chat.switch_source("codex-cli");
        assert_eq!(chat.session_id, None, "the new source has no session yet");
        chat.provider_id = "codex-cli".into();
        chat.session_id = Some("codex-session".into());
        chat.switch_source("claude-cli");
        assert_eq!(chat.session_id.as_deref(), Some("claude-session"));
        chat.provider_id = "claude-cli".into();
        chat.switch_source("codex-cli");
        assert_eq!(chat.session_id.as_deref(), Some("codex-session"));
    }

    #[test]
    fn the_same_source_as_mutis_lead_and_on_its_own_keep_separate_threads() {
        let mut chat = record();
        chat.provider_id = "codex-cli".into();
        chat.mutis_level = Some("medium".into());
        chat.session_id = Some("lead-thread".into());
        // The user leaves MutiS for the same Codex: another thread, not the lead's.
        chat.switch_source(&thread_key("codex-cli", false));
        assert_eq!(chat.session_id, None);
        chat.mutis_level = None;
        chat.session_id = Some("own-thread".into());
        chat.switch_source(&thread_key("codex-cli", true));
        assert_eq!(chat.session_id.as_deref(), Some("lead-thread"));
        chat.mutis_level = Some("medium".into());
        chat.switch_source(&thread_key("codex-cli", false));
        assert_eq!(chat.session_id.as_deref(), Some("own-thread"));
    }

    #[test]
    fn a_thread_over_the_budget_is_dropped_and_zero_turns_it_off() {
        let mut chat = record();
        chat.worker_sessions.insert("w1".into(), "w".into());
        chat.thread_tokens.insert("main".into(), 160_000);
        chat.thread_tokens.insert("worker:w1".into(), 90_000);
        assert!(chat.rotate_big_threads(0).is_empty(), "0 = off");
        assert_eq!(chat.session_id.as_deref(), Some("claude-session"));
        assert_eq!(chat.rotate_big_threads(150_000), vec![("main".to_owned(), 160_000)]);
        assert_eq!(chat.session_id, None);
        assert!(chat.worker_sessions.contains_key("w1"), "under the budget: kept");
        assert_eq!(chat.rotate_big_threads(50_000), vec![("worker:w1".to_owned(), 90_000)]);
        assert!(chat.worker_sessions.is_empty());
        assert!(chat.thread_tokens.is_empty());
    }
}
