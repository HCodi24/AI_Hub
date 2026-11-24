use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Speaking order inside a round: Claude proposes / answers, then Codex reads it and rebuts.
pub const SIDES: [Side; 2] = [Side::Claude, Side::Codex];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Codex,
    Claude,
}

impl Side {
    pub fn name(self) -> &'static str {
        match self {
            Side::Codex => "Codex",
            Side::Claude => "Claude",
        }
    }
    pub fn other(self) -> Side {
        match self {
            Side::Codex => Side::Claude,
            Side::Claude => Side::Codex,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Side::Codex => "codex",
            Side::Claude => "claude",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Turn {
    #[serde(default)]
    pub elapsed_s: u64,
    /// The answer shown to the user (the stance JSON block is removed).
    #[serde(default)]
    pub text: String,
    /// Round 2+: does this side now agree with the common direction?
    #[serde(default)]
    pub agree: Option<bool>,
    #[serde(default)]
    pub disagreements: Vec<String>,
    #[serde(default)]
    pub tokens: u64,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Round {
    pub index: u32,
    #[serde(default)]
    pub codex: Turn,
    #[serde(default)]
    pub claude: Turn,
}

impl Round {
    pub fn turn(&self, side: Side) -> &Turn {
        match side {
            Side::Codex => &self.codex,
            Side::Claude => &self.claude,
        }
    }
    pub fn turn_mut(&mut self, side: Side) -> &mut Turn {
        match side {
            Side::Codex => &mut self.codex,
            Side::Claude => &mut self.claude,
        }
    }
    /// Codex (who read Claude's latest answer) agrees, and from round 2 Claude agrees too.
    /// In round 1 Claude only proposed, so Codex's agreement alone settles it.
    pub fn converged(&self) -> bool {
        self.codex.agree == Some(true) && (self.index == 1 || self.claude.agree == Some(true))
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Verdict {
    #[serde(default)]
    pub decision: String,
    /// "codex" | "claude" | "merged"
    #[serde(default)]
    pub chosen: String,
    #[serde(default)]
    pub agreed_points: Vec<String>,
    #[serde(default)]
    pub disagreements: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub plan_steps: Vec<String>,
    #[serde(default)]
    pub tokens: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Debate {
    #[serde(default)]
    pub plan_id: Option<String>,
    #[serde(default)]
    pub chat_id: Option<String>,
    #[serde(default)]
    pub elapsed_s: u64,
    pub id: String,
    pub title: String,
    pub request: String,
    pub folder: String,
    pub created_at: u64,
    /// "running" | "done" | "error" | "stopped"
    pub status: String,
    #[serde(default)]
    pub codex_provider: String,
    #[serde(default)]
    pub codex_model: String,
    #[serde(default)]
    pub claude_provider: String,
    #[serde(default)]
    pub claude_model: String,
    #[serde(default)]
    pub judge_model: String,
    #[serde(default)]
    pub max_rounds: u32,
    #[serde(default)]
    pub rounds: Vec<Round>,
    #[serde(default)]
    pub verdict: Option<Verdict>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Debate {
    pub fn chat_summary(&self) -> String {
        let text = if let Some(verdict) = &self.verdict {
            let mut text = format!("⚖ Phán quyết\n\n{}", verdict.decision);
            for (index, step) in verdict.plan_steps.iter().enumerate() {
                text.push_str(&format!("\n{}. {}", index + 1, step));
            }
            text
        } else {
            self.error
                .clone()
                .unwrap_or_else(|| "Tranh luận đã dừng".into())
        };
        text.chars().take(1500).collect()
    }

    /// Load the latest chat each time so unrelated messages are preserved.
    pub fn save_chat(&self, store: &crate::chat::ChatStore, id: &str, start: bool) -> Result<()> {
        use crate::chat::{self, ChatMessage, ChatRequest};
        anyhow::ensure!(chat::valid_id(id), "chat_id không hợp lệ");
        let mut record = match store.get(id)? {
            Some(record) => record,
            None => chat::new_record(&ChatRequest {
                chat_id: id.into(),
                provider_id: String::new(),
                model: self.codex_model.clone(),
                folder: self.folder.clone(),
                dev_chat: false,
                permission: chat::Permission::Read,
                file_tools: Default::default(),
                effort: None,
                messages: Vec::new(),
                plan_id: None,
                task_id: None,
                task_kind: None,
            }),
        };
        if start {
            if record.messages.is_empty() {
                record.title = self.title.clone();
            }
            let user: ChatMessage = serde_json::from_value(serde_json::json!({
                "role": "user", "content": self.request
            }))?;
            let assistant: ChatMessage = serde_json::from_value(serde_json::json!({
                "role": "assistant", "content": "", "debate_id": self.id
            }))?;
            record.messages.extend([user, assistant]);
        } else if let Some(message) = record
            .messages
            .iter_mut()
            .find(|message| message.debate_id.as_deref() == Some(&self.id))
        {
            message.content = self.chat_summary();
        }
        chat::save_record(store, &mut record)
    }

    pub fn total_tokens(&self) -> u64 {
        self.rounds
            .iter()
            .map(|round| round.codex.tokens + round.claude.tokens)
            .sum::<u64>()
            + self.verdict.as_ref().map_or(0, |verdict| verdict.tokens)
    }
}
