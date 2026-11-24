use super::types::Verdict;
use super::*;

#[test]
fn debate_chat_append_roundtrip_and_update_preserve_other_messages() {
    let directory = std::env::temp_dir().join(format!("debate-chat-{}", uuid::Uuid::new_v4()));
    let store = crate::chat::ChatStore::new(directory.clone());
    let id = uuid::Uuid::new_v4().to_string();
    let mut debate = Debate {
        id: uuid::Uuid::new_v4().to_string(),
        title: "Kiểm tra tranh luận".into(),
        request: "Chọn hướng nào?".into(),
        folder: "project".into(),
        ..Default::default()
    };
    debate.save_chat(&store, &id, true).unwrap();
    let record = store.get(&id).unwrap().unwrap();
    assert_eq!(record.title, debate.title);
    assert_eq!(record.folder, debate.folder);
    assert_eq!(record.messages.len(), 2);
    assert_eq!(record.messages[0].content, debate.request);
    assert_eq!(
        record.messages[1].debate_id.as_deref(),
        Some(debate.id.as_str())
    );
    assert!(record.messages[1].content.is_empty());
    let second = Debate {
        id: uuid::Uuid::new_v4().to_string(),
        ..debate.clone()
    };
    second.save_chat(&store, &id, true).unwrap();
    debate.verdict = Some(Verdict {
        decision: "Gộp hai hướng".into(),
        plan_steps: vec!["Kiểm tra cache".into(), "Chạy kiểm thử".into()],
        ..Default::default()
    });
    debate.save_chat(&store, &id, false).unwrap();
    let record = store.get(&id).unwrap().unwrap();
    assert_eq!(record.messages.len(), 4);
    assert!(record.messages[1].content.contains("Gộp hai hướng"));
    assert!(record.messages[1].content.contains("1. Kiểm tra cache"));
    assert!(record.messages[3].content.is_empty());
    debate.verdict = None;
    debate.error = Some("Lỗi trọng tài".into());
    debate.save_chat(&store, &id, false).unwrap();
    assert_eq!(
        store.get(&id).unwrap().unwrap().messages[1].content,
        "Lỗi trọng tài"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn debate_summary_is_bounded_unicode_and_invalid_chat_id_is_rejected() {
    let directory =
        std::env::temp_dir().join(format!("debate-invalid-{}", uuid::Uuid::new_v4()));
    let store = crate::chat::ChatStore::new(directory.clone());
    let debate = Debate {
        verdict: Some(Verdict {
            decision: "Việt".repeat(1000),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(debate.chat_summary().chars().count(), 1500);
    assert!(debate.save_chat(&store, "../invalid", true).is_err());
    assert!(!directory.exists());
    let old: crate::chat::ChatMessage =
        serde_json::from_str(r#"{"role":"assistant","content":"cũ"}"#).unwrap();
    assert!(old.debate_id.is_none());
    assert!(serde_json::to_value(old)
        .unwrap()
        .get("debate_id")
        .is_none());
}

#[test]
fn stance_is_read_from_the_last_json_block_and_hidden_from_the_text() {
    let text =
        "Tôi đồng ý gộp hai hướng.\n\n```json\n{\"agree\": true, \"disagreements\": []}\n```";
    let (shown, agree, disagreements) = split_stance(text);
    assert_eq!(shown, "Tôi đồng ý gộp hai hướng.");
    assert_eq!(agree, Some(true));
    assert!(disagreements.is_empty());
}

#[test]
fn stance_without_fence_and_missing_stance_are_handled() {
    let (shown, agree, disagreements) =
        split_stance("Chưa đồng ý. {\"agree\": false, \"disagreements\": [\"cache\", \" \"]}");
    assert_eq!(shown, "Chưa đồng ý.");
    assert_eq!(agree, Some(false));
    assert_eq!(disagreements, ["cache"]);
    let (shown, agree, _) = split_stance("Chỉ có chữ, không có JSON.");
    assert_eq!(shown, "Chỉ có chữ, không có JSON.");
    assert_eq!(agree, None);
}

#[test]
fn convergence_needs_both_sides_from_round_two() {
    let agree = Turn {
        agree: Some(true),
        ..Default::default()
    };
    let round1 = Round {
        index: 1,
        codex: agree.clone(),
        claude: Turn::default(),
    };
    assert!(
        round1.converged(),
        "round 1: Claude only proposed, Codex agreeing settles it"
    );
    let codex_disagrees = Round {
        index: 1,
        codex: Turn {
            agree: Some(false),
            ..Default::default()
        },
        claude: Turn::default(),
    };
    assert!(!codex_disagrees.converged());
    let round2 = Round {
        index: 2,
        codex: agree.clone(),
        claude: agree.clone(),
    };
    assert!(round2.converged());
    let split = Round {
        index: 2,
        codex: agree,
        claude: Turn {
            agree: Some(false),
            ..Default::default()
        },
    };
    assert!(!split.converged());
}

#[test]
fn verdict_is_parsed_and_unknown_choice_becomes_merged() {
    let verdict = parse_verdict(
        "Kết luận:\n{\"decision\":\"Dùng cách của Codex\",\"chosen\":\"???\",\"plan_steps\":[\"a\",\"b\"]}",
    )
    .unwrap();
    assert_eq!(verdict.decision, "Dùng cách của Codex");
    assert_eq!(verdict.chosen, "merged");
    assert_eq!(verdict.plan_steps, ["a", "b"]);
    assert!(parse_verdict("{\"decision\":\"\"}").is_none());
}

#[test]
fn judge_prompt_contains_both_sides_and_stances() {
    let debate = Debate {
        request: "Sửa lỗi X".into(),
        rounds: vec![Round {
            index: 2,
            codex: Turn {
                text: "Đề xuất A".into(),
                agree: Some(false),
                disagreements: vec!["khoá".into()],
                ..Default::default()
            },
            claude: Turn {
                text: "Đề xuất B".into(),
                agree: Some(true),
                ..Default::default()
            },
        }],
        ..Default::default()
    };
    let prompt = judge_prompt(&debate, None);
    assert!(prompt.contains("Sửa lỗi X"));
    assert!(prompt.contains("Đề xuất A") && prompt.contains("Đề xuất B"));
    assert!(prompt.contains("chưa đồng ý") && prompt.contains("khoá"));
    assert!(prompt.contains("KHÔNG tham gia"));
}

#[test]
fn token_log_lines_are_summed() {
    assert_eq!(
        tokens_from_log("Token: vào 1200 (cache 300), ra 45 (suy luận 10)"),
        Some(1245)
    );
    assert_eq!(
        tokens_from_log("Token: vào 7, ra 3, chi phí $0.01"),
        Some(10)
    );
    assert_eq!(tokens_from_log("Đọc file a.rs"), None);
}

#[test]
fn plan_request_carries_the_verdict_steps() {
    let debate = Debate {
        request: "Làm Y".into(),
        verdict: Some(Verdict {
            decision: "Gộp hai hướng".into(),
            plan_steps: vec!["Bước một".into()],
            ..Default::default()
        }),
        ..Default::default()
    };
    let text = plan_request(&debate);
    assert!(text.starts_with("Làm Y"));
    assert!(text.contains("Gộp hai hướng") && text.contains("1. Bước một"));
}
