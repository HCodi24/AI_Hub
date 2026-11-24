use crate::chat::ChatMessage;
use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn api_images_with_limit(messages: &mut [ChatMessage]) -> bool {
    let mut total = messages
        .iter()
        .flat_map(|m| &m.attachments)
        .filter_map(|a| fs::metadata(&a.path).ok())
        .map(|m| m.len().div_ceil(3).saturating_mul(4))
        .sum::<u64>();
    let mut dropped = false;
    for message in messages.iter_mut().filter(|m| m.role == "user") {
        if total < 15 * 1024 * 1024 {
            break;
        }
        while total >= 15 * 1024 * 1024 && !message.attachments.is_empty() {
            let attachment = message.attachments.remove(0);
            total = total.saturating_sub(
                fs::metadata(attachment.path)
                    .map(|m| m.len().div_ceil(3).saturating_mul(4))
                    .unwrap_or(0),
            );
            dropped = true;
        }
    }
    dropped
}

pub(crate) fn image_block(attachment: &crate::chat::ChatAttachment) -> Result<Value> {
    let bytes = fs::read(&attachment.path).context("Không thể đọc ảnh đính kèm")?;
    Ok(
        json!({"type":"image","source":{"type":"base64","media_type":attachment.mime,"data":crate::api::attachments::encode_base64(&bytes)}}),
    )
}

/// The message whose images a CLI should be pointed at: only the user's newest message. An image from an earlier
/// turn must not be re-attached to an unrelated question (the CLI would answer about the old image).
pub(super) fn message_with_new_images(messages: &[ChatMessage]) -> Option<&ChatMessage> {
    messages.last().filter(|m| m.role == "user" && !m.attachments.is_empty())
}

pub(crate) fn latest_chat_attachment_dir(messages: &[ChatMessage]) -> Option<PathBuf> {
    message_with_new_images(messages)?
        .attachments
        .first()?
        .path
        .parse::<PathBuf>()
        .ok()?
        .parent()
        .map(|path| crate::core::text::strip_verbatim(path.to_path_buf()))
}

pub(crate) fn latest_image_paths(
    messages: &[ChatMessage],
    expected_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let expected_dir = fs::canonicalize(expected_dir).context("Thư mục ảnh chat không hợp lệ")?;
    let Some(message) = message_with_new_images(messages) else {
        return Ok(Vec::new());
    };
    message
        .attachments
        .iter()
        .map(|a| {
            let path = fs::canonicalize(&a.path).context("Không thể giải quyết đường dẫn ảnh")?;
            if !path.starts_with(&expected_dir) || !path.is_file() {
                anyhow::bail!("Đường dẫn ảnh nằm ngoài thư mục đính kèm chat");
            }
            Ok(crate::core::text::strip_verbatim(path))
        })
        .collect()
}
