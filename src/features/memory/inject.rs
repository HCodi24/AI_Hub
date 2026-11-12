//! Only the current project's `core.md` is ever injected automatically.
use super::{store, MemoryConfig, Store, INSTRUCTION};
use std::path::Path;

pub fn memory_context(config: &Path, folder: &str, settings: &MemoryConfig) -> String {
    // Reading an untouched project must not create a store on disk.
    if !settings.enabled || !store::exists(config, folder) {
        return String::new();
    }
    Store::open(config, folder, settings).map(|s| s.core()).unwrap_or_default()
}

pub fn inject(request: &mut crate::providers::ProviderRequest, context: &str) {
    let cli = matches!(request.provider.kind.as_str(), "codex_cli" | "claude_cli");
    if !context.is_empty() && (!cli || request.session_id.is_none()) {
        let message = if cli {
            request.messages.iter_mut().rev().find(|m| m.role == "user")
        } else {
            request.messages.iter_mut().find(|m| m.role == "user")
        };
        if let Some(m) = message {
            m.content = format!("{context}\n\n{}", m.content);
        }
    }
    if let Some(m) = request.messages.iter_mut().rev().find(|m| m.role == "user") {
        m.content.push_str(INSTRUCTION);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provider_copy_injection_obeys_session_and_message_positions() {
        for kind in ["codex_cli", "claude_cli", "openai_compat", "deepseek_agent", "anthropic"] {
            for session in [None, Some("session".to_owned())] {
                let messages: Vec<crate::chat::ChatMessage> = serde_json::from_value(serde_json::json!([
                    {"role":"user","content":"first"}, {"role":"assistant","content":"answer"},
                    {"role":"user","content":"latest"}
                ]))
                .unwrap();
                let mut request = crate::providers::ProviderRequest {
                    provider: crate::config::ProviderConfig { kind: kind.into(), ..Default::default() },
                    model: "model".into(),
                    folder: "folder".into(),
                    permission: Default::default(),
                    effort: Default::default(),
                    messages: messages.clone(),
                    session_id: session.clone(),
                    mutis: Default::default(),
                };
                inject(&mut request, "MEMORY_CONTEXT");
                let cli = matches!(kind, "codex_cli" | "claude_cli");
                assert_eq!(request.messages[0].content.contains("MEMORY_CONTEXT"), !cli);
                assert_eq!(request.messages[2].content.contains("MEMORY_CONTEXT"), cli && session.is_none());
                assert!(request.messages[2].content.ends_with(INSTRUCTION));
                assert_eq!(request.messages[1].content, "answer");
                assert_eq!(messages[0].content, "first");
                assert_eq!(messages[2].content, "latest");
            }
        }
    }
}
