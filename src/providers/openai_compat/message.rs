use anyhow::{Context, Result};
use serde_json::{json, Value};

pub(crate) fn openai_message(message: &crate::chat::ChatMessage) -> Result<Value> {
    if message.attachments.is_empty() {
        return Ok(json!({"role":message.role,"content":message.content}));
    }
    let mut content = vec![json!({"type":"text","text":message.content})];
    for attachment in &message.attachments {
        let bytes = std::fs::read(&attachment.path).context("Không thể đọc ảnh đính kèm")?;
        content.push(json!({"type":"image_url","image_url":{"url":format!("data:{};base64,{}", attachment.mime, crate::api::attachments::encode_base64(&bytes))}}));
    }
    Ok(json!({"role":message.role,"content":content}))
}
