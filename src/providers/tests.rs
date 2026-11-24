use super::images::message_with_new_images;
use super::{
    api_reasoning_effort, latest_chat_attachment_dir, retry_delay, retryable_status, ChatMessage,
    Effort,
};

mod retry_tests {
    use super::{retry_delay, retryable_status};
    #[test]
    fn deepseek_flash_effort_preserves_three_levels() {
        use super::{api_reasoning_effort, Effort};
        for model in ["DeepSeek-V4.1-Flash", "deepseek-v4.1-flash", "deepseek-flash"] {
            for (effort, expected) in [(Effort::Low, "low"), (Effort::High, "high"), (Effort::Max, "max"),
                (Effort::Default, "high"), (Effort::Medium, "high"), (Effort::Xhigh, "high")] {
                assert_eq!(api_reasoning_effort(model, effort), Some(expected));
            }
        }
        assert_eq!(api_reasoning_effort("other-model", Effort::Default), None);
        assert_eq!(api_reasoning_effort("other-model", Effort::Medium), Some("medium"));
        assert_eq!(api_reasoning_effort("other-model", Effort::Max), Some("high"));
    }
    #[test]
    fn retries_only_transient_statuses() {
        for status in [429, 500, 502, 503, 504] {
            assert!(retryable_status(
                reqwest::StatusCode::from_u16(status).unwrap()
            ));
        }
        for status in [400, 401, 404, 501] {
            assert!(!retryable_status(
                reqwest::StatusCode::from_u16(status).unwrap()
            ));
        }
    }
    #[test]
    fn backoff_doubles_and_caps_jitter() {
        assert_eq!(retry_delay(2000, 0, 0).as_millis(), 2000);
        assert_eq!(retry_delay(2000, 1, 250).as_millis(), 4250);
        assert_eq!(retry_delay(2000, 2, 999).as_millis(), 8250);
    }
}

mod image_scope_tests {
    use super::*;

    fn message(role: &str, images: bool) -> ChatMessage {
        let attachments = if images { serde_json::json!([{"id": "i", "mime": "image/png", "path": "C:/x/i.png"}]) } else { serde_json::json!([]) };
        serde_json::from_value(serde_json::json!({"role": role, "content": "c", "attachments": attachments})).unwrap()
    }

    #[test]
    fn only_the_newest_user_message_brings_images() {
        let old_image = [message("user", true), message("assistant", false), message("user", false)];
        assert!(message_with_new_images(&old_image).is_none(), "an old image must not follow later questions");
        assert!(latest_chat_attachment_dir(&old_image).is_none());
        let new_image = [message("user", false), message("assistant", false), message("user", true)];
        assert!(message_with_new_images(&new_image).is_some());
    }
}
