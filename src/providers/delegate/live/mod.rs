use crate::{
    chat::{Effort, Permission},
    config::{McpServerConfig, MutisConfig, ProviderConfig},
    providers::{quota, schedule::Actor, ProviderEvent},
};
use std::sync::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

mod api;
mod backend;
mod check;
mod cli;

/// What one round needs to talk to the worker and the checker; shared by the round loop and the checker's task.
pub(super) struct Live {
    pub(super) worker: ProviderConfig,
    pub(super) model: String,
    pub(super) effort: Effort,
    pub(super) worker_permission: Permission,
    pub(super) worker_count: usize,
    /// CLI thread per worker slot, kept for every task of this reply.
    pub(super) worker_cli_sessions: [Mutex<Option<String>>; super::WORKER_SLOTS as usize],
    /// Input tokens of each Codex worker thread's last model call, kept in the chat record.
    pub(super) worker_cli_tokens: [std::sync::atomic::AtomicU64; super::WORKER_SLOTS as usize],
    pub(super) folder: String,
    pub(super) mutis: MutisConfig,
    pub(super) events: mpsc::Sender<ProviderEvent>,
    pub(super) cancel: CancellationToken,
    pub(super) checker: Option<(ProviderConfig, String, Effort)>,
    pub(super) checker_permission: Permission,
    /// Key of the worker's conversation: one per chat, so every task and retry continues what the worker already read.
    pub(super) worker_session: String,
    /// The checker's own thread (sources that keep one): resumed for every check, kept in the chat record between messages.
    pub(super) checker_session: Mutex<Option<String>>,
    /// The checker processes one task at a time.
    pub(super) tester_busy: tokio::sync::Mutex<()>,
    /// The checker's last "Token:" log line, held until `announce_check` shows it in the Tester turn.
    pub(super) checker_tokens: Mutex<Option<String>>,
    /// Whether the coordinator may edit files, i.e. can take a task over when the attempts run out.
    pub(super) can_edit: bool,
    /// CodeGraph's MCP server as the lead got it (indexed project, option on); None = no role gets it.
    pub(super) codegraph: Option<McpServerConfig>,
}

/// Said to every role that gets the CodeGraph tool, so it searches the index before reading files one by one.
const CODEGRAPH_NOTE: &str = "\nThe codegraph MCP tool is attached: use codegraph_explore to find and read code before opening files one by one.";

pub(super) fn cli_mcp(codegraph: Option<&McpServerConfig>, kind: &str) -> Vec<McpServerConfig> {
    if matches!(kind, "codex_cli" | "claude_cli") { codegraph.cloned().into_iter().collect() } else { Vec::new() }
}

const CLI_WORK_NOTE: &str = concat!(
    "
You work directly in the project folder. Edit only what the task needs. ",
    "After editing run only quick checks of the files you changed (format check, lint, type check). ",
    "Do not set up a build environment, and do not run end-to-end tests or the whole project check: a separate checker does that. ",
    "Read and edit only inside the project folder; do not look into other folders or repositories."
);

impl Live {
    /// The tools of one delegated role: only the CLI sources can call an MCP server, and none of them gets the lead's own servers.
    pub(super) fn mcp_for(&self, kind: &str) -> Vec<McpServerConfig> {
        cli_mcp(self.codegraph.as_ref(), kind)
    }

    /// The prompt of a CLI role, with the CodeGraph hint when the tool is attached.
    pub(super) fn with_codegraph_note(&self, prompt: String, mcp: &[McpServerConfig]) -> String {
        if mcp.is_empty() { prompt } else { prompt + CODEGRAPH_NOTE }
    }

    /// Whether the worker runs as a CLI thread (Codex, Claude, Antigravity) instead of the API agent loop.
    /// The source an actor calls, for its usage limit: the worker slots share one, the tester has its own.
    pub(super) fn source_of(&self, actor: Actor) -> Option<&str> {
        match actor {
            Actor::Tester => self.checker.as_ref().map(|(provider, _, _)| provider.id.as_str()),
            _ => Some(self.worker.id.as_str()),
        }
    }

    pub(super) fn worker_is_cli(&self) -> bool {
        matches!(self.worker.kind.as_str(), "codex_cli" | "claude_cli" | "antigravity_cli")
    }

    /// Name shown for the worker: the Codex or Claude CLI family, or the source's own name.
    /// The task board adds the exact model after it.
    pub(super) fn worker_label(&self) -> String {
        match self.worker.kind.as_str() {
            "codex_cli" => "Codex".to_owned(),
            "claude_cli" => "Claude".to_owned(),
            _ => self.worker.name.clone(),
        }
    }
}
