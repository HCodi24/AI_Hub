use anyhow::Result;
use serde_json::{json, Value};

pub(super) fn anthropic_content(message: &crate::chat::ChatMessage) -> Result<Vec<Value>> {
    let mut blocks = Vec::new();
    for attachment in &message.attachments {
        blocks.push(crate::providers::image_block(attachment)?);
    }
    if !message.content.is_empty() {
        blocks.push(json!({"type":"text","text":message.content}));
    }
    Ok(blocks)
}
