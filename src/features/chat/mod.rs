use axum::response::sse::Event;
use serde_json::Value;

mod message;
mod permission;
mod record;
mod store;

pub use message::{ChatAttachment, ChatMessage, STOPPED_NOTE};
pub use permission::{agent_permission, file_tools_for, parse_effort, Effort, FileTools, Permission};
pub use record::{new_record, thread_key, ChatRecord, ChatRequest, ChatSummary};
pub use store::{config_chat_dir, save_record, valid_id, ChatStore};

#[cfg(test)]
mod tests;

pub fn sse_event(name: &'static str, data: &Value) -> Event {
    let encoded = serde_json::to_string(data).unwrap_or_else(|_| "{}".to_owned());
    Event::default().event(name).data(encoded)
}
