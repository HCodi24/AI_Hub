use super::{anthropic_content, auth_headers};
use crate::chat::{ChatAttachment, ChatMessage};
use std::fs;
use uuid::Uuid;
#[test]
fn anthropic_builder_places_image_before_text() {
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
    let blocks = anthropic_content(&message).unwrap();
    assert_eq!(blocks[0]["type"], "image");
    assert_eq!(blocks[0]["source"]["media_type"], "image/png");
    assert_eq!(blocks[1]["type"], "text");
    let _ = fs::remove_file(path);
}

#[test]
fn auth_headers_sets_key_and_version() {
    let headers = auth_headers("k");
    assert!(headers.contains(&("x-api-key".into(), "k".into())));
    assert!(headers.contains(&("anthropic-version".into(), "2023-06-01".into())));
}
