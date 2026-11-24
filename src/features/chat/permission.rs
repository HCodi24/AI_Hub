use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Permission {
    Read,
    #[default]
    Write,
    /// CLI providers run commands themselves; API sources get run_command.
    Full,
}

/// File tools an API-key source may use in agent mode.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FileTools {
    #[default]
    None,
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Effort {
    #[default]
    Default,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
    Max,
    Ultra,
}

pub fn parse_effort(value: &str) -> Option<Effort> {
    match value {
        "default" => Some(Effort::Default),
        "none" => Some(Effort::None),
        "minimal" => Some(Effort::Minimal),
        "low" => Some(Effort::Low),
        "medium" => Some(Effort::Medium),
        "high" => Some(Effort::High),
        "xhigh" => Some(Effort::Xhigh),
        "max" => Some(Effort::Max),
        "ultra" => Some(Effort::Ultra),
        _ => None,
    }
}

/// The file tools of a chat at the MutiS source, whose lead is only known on the server: API-key agent sources get them
/// (read-only when the chat permission is read), CLI sources edit files themselves.
pub fn file_tools_for(provider: &crate::config::ProviderConfig, permission: Permission) -> FileTools {
    let agent = provider.kind == "anthropic" || (provider.kind == "openai_compat" && provider.agent_capable);
    match (agent, permission) {
        (false, _) => FileTools::None,
        (true, Permission::Read) => FileTools::Read,
        (true, _) => FileTools::Write,
    }
}

/// Decide how a chat message is run for a source:
/// - `Ok(None)`: normal chat (no file tools);
/// - `Ok(Some(permission))`: read-only (`Read`), file-write (`Write`), or command-enabled (`Full`) agent mode;
/// - `Err(message)`: the user asked for file tools on a source that does not allow them.
pub fn agent_permission(
    provider: &crate::config::ProviderConfig,
    file_tools: FileTools,
    permission: Permission,
) -> Result<Option<Permission>, &'static str> {
    if file_tools == FileTools::None {
        return Ok(None);
    }
    if provider.kind != "anthropic" && (provider.kind != "openai_compat" || !provider.agent_capable)
    {
        return Err("Bật «Cho phép AI đọc/sửa file» cho nguồn này trong Cài đặt");
    }
    Ok(Some(match file_tools {
        FileTools::Read => Permission::Read,
        FileTools::Write if permission == Permission::Full => Permission::Full,
        FileTools::Write => Permission::Write,
        FileTools::None => unreachable!(),
    }))
}
