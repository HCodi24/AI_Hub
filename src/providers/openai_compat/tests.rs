use super::{images_rejected, openai_message};
use crate::chat::{ChatAttachment, ChatMessage};
use std::fs;
use uuid::Uuid;
#[test]
fn openai_builder_adds_data_image_after_text() {
    let path = std::env::temp_dir().join(format!("{}.png", Uuid::new_v4()));
    fs::write(&path, b"pixels").unwrap();
    let message = ChatMessage {
        debate_id: None,
        plan_id: None,
        role: "user".into(),
        content: "Look".into(),
        attachments: vec![ChatAttachment {
            id: "id".into(),
            mime: "image/png".into(),
            path: path.to_string_lossy().into_owned(),
        }],
        reasoning: String::new(),
        logs: vec![],
        diff: String::new(),
        files: Vec::new(),
        tasks: None,
        provider: String::new(),
        model: String::new(),
        error: None,
    };
    let value = openai_message(&message).unwrap();
    assert_eq!(value["content"][0]["type"], "text");
    assert_eq!(
        value["content"][1]["image_url"]["url"],
        "data:image/png;base64,cGl4ZWxz"
    );
    let _ = fs::remove_file(path);
}

#[test]
fn images_rejected_checks_attachments_and_keywords() {
    let plain = ChatMessage {
        debate_id: None,
        plan_id: None,
        role: "user".into(),
        content: "Hello".into(),
        attachments: vec![],
        reasoning: String::new(),
        logs: vec![],
        diff: String::new(),
        files: Vec::new(),
        tasks: None,
        provider: String::new(),
        model: String::new(),
        error: None,
    };
    assert!(!images_rejected(&[plain], "image"));

    let with_image = ChatMessage {
        debate_id: None,
        plan_id: None,
        role: "user".into(),
        content: "Look".into(),
        attachments: vec![ChatAttachment {
            id: "id".into(),
            mime: "image/png".into(),
            path: "sample.png".into(),
        }],
        reasoning: String::new(),
        logs: vec![],
        diff: String::new(),
        files: Vec::new(),
        tasks: None,
        provider: String::new(),
        model: String::new(),
        error: None,
    };
    assert!(images_rejected(
        std::slice::from_ref(&with_image),
        "Vision not supported"
    ));
    assert!(!images_rejected(&[with_image], "rate limited"));
}
