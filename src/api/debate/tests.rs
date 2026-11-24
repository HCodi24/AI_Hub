use crate::features::debate::Debate;
use serde_json::json;

#[test]
fn debates_saved_before_chats_or_plans_still_load() {
    let old: Debate = serde_json::from_value(json!({
        "id": "d", "title": "t", "request": "r", "folder": "f", "created_at": 1, "status": "done"
    }))
    .unwrap();
    assert!(old.plan_id.is_none() && old.chat_id.is_none());
}
