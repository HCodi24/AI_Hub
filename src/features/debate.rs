//! "⚖ Tranh luận": Codex and Claude debate a problem (read-only), then a SECOND, fresh Claude session judges.
//!
//! Round 1: both propose independently. Round 2+: each critiques the other's proposal and revises its own,
//! ending with a stance block. The debate stops early when both agree. The judge never took part in the debate,
//! so it is not biased towards its own proposal.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf};

pub const SIDES: [Side; 2] = [Side::Codex, Side::Claude];

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
    /// Both sides explicitly agree (only meaningful from round 2, after they have read each other).
    pub fn converged(&self) -> bool {
        self.index >= 2 && self.codex.agree == Some(true) && self.claude.agree == Some(true)
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
    pub fn total_tokens(&self) -> u64 {
        self.rounds
            .iter()
            .map(|round| round.codex.tokens + round.claude.tokens)
            .sum::<u64>()
            + self.verdict.as_ref().map_or(0, |verdict| verdict.tokens)
    }
}

const STANCE_RULE: &str = concat!(
    "Kết thúc câu trả lời bằng ĐÚNG một khối JSON (trong ```json ... ```) dạng ",
    "{\"agree\": true|false, \"disagreements\": [\"...\"]}. ",
    "agree = true nếu sau khi xem xét bạn đồng ý với hướng giải quyết chung với bên kia ",
    "(có thể đã gộp ý của nhau); disagreements = những điểm bạn VẪN chưa đồng ý (để trống nếu đồng ý)."
);

/// Round 1: independent proposal.
pub fn proposal_prompt(side: Side, request: &str) -> String {
    format!(
        "Bạn là {me} trong một cuộc tranh luận kỹ thuật với {other} để tìm hướng giải quyết ĐÚNG nhất.\n\
         Bạn chỉ được ĐỌC code trong thư mục (không sửa file, không chạy lệnh làm thay đổi gì).\n\n\
         Vấn đề cần giải quyết:\n{request}\n\n\
         Hãy đọc phần code liên quan rồi đưa ra đề xuất của bạn, gồm:\n\
         1. Hiểu vấn đề (nguyên nhân / yêu cầu thật sự).\n\
         2. Hướng giải quyết và lý do chọn.\n\
         3. Các bước cụ thể (file, hàm cần đổi).\n\
         4. Rủi ro và cách kiểm tra.\n\
         Viết ngắn gọn, bằng tiếng Việt.",
        me = side.name(),
        other = side.other().name(),
    )
}

/// Round 2+: critique the other side's latest proposal and revise.
pub fn critique_prompt(
    side: Side,
    request: &str,
    own: &str,
    other_text: &str,
    round: u32,
) -> String {
    format!(
        "Vòng {round} của cuộc tranh luận. Bạn là {me}. Vấn đề:\n{request}\n\n\
         Đề xuất trước của BẠN:\n<<<\n{own}\n>>>\n\n\
         Đề xuất mới nhất của {other}:\n<<<\n{other_text}\n>>>\n\n\
         Hãy phản biện thẳng thắn và công bằng (vẫn chỉ đọc code, không sửa file):\n\
         - Chỗ sai, thiếu hoặc rủi ro trong đề xuất của {other} (kiểm chứng bằng code nếu cần).\n\
         - Điểm hay của {other} mà bạn nên lấy.\n\
         - Đề xuất đã sửa của bạn (có thể gộp ý của cả hai).\n\
         Đừng đồng ý chỉ để kết thúc nhanh: chỉ đồng ý khi bạn thật sự thấy hướng đó đúng.\n\
         {STANCE_RULE}",
        me = side.name(),
        other = side.other().name(),
    )
}

/// The judge: a second, fresh Claude session that did not take part in the debate.
pub fn judge_prompt(debate: &Debate) -> String {
    let mut transcript = String::new();
    for round in &debate.rounds {
        for side in SIDES {
            let turn = round.turn(side);
            transcript.push_str(&format!(
                "\n=== Vòng {} · {} ===\n{}\n",
                round.index,
                side.name(),
                if turn.error.is_some() && turn.text.is_empty() {
                    "(không trả lời được)"
                } else {
                    turn.text.as_str()
                }
            ));
            if let Some(agree) = turn.agree {
                transcript.push_str(&format!(
                    "[Lập trường: {}; còn bất đồng: {}]\n",
                    if agree {
                        "đồng ý"
                    } else {
                        "chưa đồng ý"
                    },
                    if turn.disagreements.is_empty() {
                        "không".to_owned()
                    } else {
                        turn.disagreements.join("; ")
                    }
                ));
            }
        }
    }
    format!(
        "Bạn là TRỌNG TÀI độc lập (bạn KHÔNG tham gia tranh luận). Codex và Claude đã tranh luận về vấn đề sau:\n\
         {request}\n\nBiên bản tranh luận:{transcript}\n\n\
         Hãy đánh giá công bằng dựa trên lập luận và bằng chứng (có thể đọc code để kiểm chứng; không sửa file). \
         Chọn hướng ĐÚNG nhất: của Codex, của Claude, hoặc gộp. Trả lời CHỈ bằng một JSON:\n\
         {{\"decision\": \"kết luận ngắn gọn: làm gì và vì sao\", \"chosen\": \"codex|claude|merged\", \
         \"agreed_points\": [\"...\"], \"disagreements\": [\"điểm còn tranh cãi và bạn phân xử ra sao\"], \
         \"risks\": [\"...\"], \"plan_steps\": [\"bước 1 ...\", \"bước 2 ...\"]}}",
        request = debate.request,
    )
}

/// Find the stance JSON (the last ```json block, or the last `{...}` containing "agree") and remove it from
/// the text shown to the user.
pub fn split_stance(text: &str) -> (String, Option<bool>, Vec<String>) {
    let candidate = last_fenced_json(text).or_else(|| last_object_with(text, "\"agree\""));
    let Some((start, end, json)) = candidate else {
        return (text.trim().to_owned(), None, Vec::new());
    };
    let Ok(value) = serde_json::from_str::<Value>(&json) else {
        return (text.trim().to_owned(), None, Vec::new());
    };
    let agree = value["agree"].as_bool();
    let disagreements = value["disagreements"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item.as_str())
        .map(|item| item.trim().to_owned())
        .filter(|item| !item.is_empty())
        .collect();
    let shown = format!("{}{}", &text[..start], &text[end..]);
    (shown.trim().to_owned(), agree, disagreements)
}

fn last_fenced_json(text: &str) -> Option<(usize, usize, String)> {
    let start = text.rfind("```json")?;
    let body_start = start + "```json".len();
    let close = text[body_start..].find("```")? + body_start;
    Some((start, close + 3, text[body_start..close].trim().to_owned()))
}

fn last_object_with(text: &str, needle: &str) -> Option<(usize, usize, String)> {
    let at = text.rfind(needle)?;
    let start = text[..at].rfind('{')?;
    let end = text[at..].find('}')? + at + 1;
    Some((start, end, text[start..end].to_owned()))
}

/// Parse the judge's JSON answer.
pub fn parse_verdict(text: &str) -> Option<Verdict> {
    let json = crate::core::text::extract_json(text)?;
    let value: Value = serde_json::from_str(json).ok()?;
    let list = |key: &str| -> Vec<String> {
        value[key]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str().map(str::to_owned))
            .collect()
    };
    let decision = value["decision"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_owned();
    if decision.is_empty() {
        return None;
    }
    let chosen = match value["chosen"].as_str().unwrap_or_default() {
        "codex" => "codex",
        "claude" => "claude",
        _ => "merged",
    };
    Some(Verdict {
        decision,
        chosen: chosen.to_owned(),
        agreed_points: list("agreed_points"),
        disagreements: list("disagreements"),
        risks: list("risks"),
        plan_steps: list("plan_steps"),
        tokens: 0,
    })
}

/// Text handed to "Chia việc" when the user accepts the verdict.
pub fn plan_request(debate: &Debate) -> String {
    let Some(verdict) = &debate.verdict else {
        return debate.request.clone();
    };
    let mut text = format!(
        "{}\n\nKết luận sau tranh luận Codex ↔ Claude (trọng tài độc lập):\n{}\n",
        debate.request, verdict.decision
    );
    if !verdict.plan_steps.is_empty() {
        text.push_str("\nCác bước đã thống nhất:\n");
        for (index, step) in verdict.plan_steps.iter().enumerate() {
            text.push_str(&format!("{}. {}\n", index + 1, step));
        }
    }
    if !verdict.risks.is_empty() {
        text.push_str(&format!(
            "\nRủi ro cần lưu ý: {}\n",
            verdict.risks.join("; ")
        ));
    }
    text
}

/// Sum "Token: vào X ..., ra Y ..." log lines.
pub fn tokens_from_log(line: &str) -> Option<u64> {
    let rest = line.strip_prefix("Token: vào ")?;
    let input: u64 = rest
        .split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()?;
    let output = rest
        .split("ra ")
        .nth(1)
        .and_then(|part| part.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|digits| digits.parse::<u64>().ok())
        .unwrap_or(0);
    Some(input + output)
}

#[derive(Clone)]
pub struct DebateStore {
    directory: PathBuf,
}

impl DebateStore {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
    fn path(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id).context("id tranh luận không hợp lệ")?;
        Ok(self.directory.join(format!("{id}.json")))
    }
    pub fn save(&self, debate: &Debate) -> Result<()> {
        fs::create_dir_all(&self.directory)?;
        let json = serde_json::to_string_pretty(debate)?;
        fs::write(self.path(&debate.id)?, json)?;
        Ok(())
    }
    pub fn get(&self, id: &str) -> Result<Option<Debate>> {
        let path = self.path(id)?;
        if !path.exists() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&fs::read_to_string(path)?)?))
    }
    pub fn list(&self) -> Result<Vec<Debate>> {
        let mut debates = Vec::new();
        if let Ok(entries) = fs::read_dir(&self.directory) {
            for entry in entries.flatten() {
                if let Ok(text) = fs::read_to_string(entry.path()) {
                    if let Ok(debate) = serde_json::from_str::<Debate>(&text) {
                        debates.push(debate);
                    }
                }
            }
        }
        debates.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(debates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stance_is_read_from_the_last_json_block_and_hidden_from_the_text() {
        let text =
            "Tôi đồng ý gộp hai hướng.\n\n```json\n{\"agree\": true, \"disagreements\": []}\n```";
        let (shown, agree, disagreements) = split_stance(text);
        assert_eq!(shown, "Tôi đồng ý gộp hai hướng.");
        assert_eq!(agree, Some(true));
        assert!(disagreements.is_empty());
    }

    #[test]
    fn stance_without_fence_and_missing_stance_are_handled() {
        let (shown, agree, disagreements) =
            split_stance("Chưa đồng ý. {\"agree\": false, \"disagreements\": [\"cache\", \" \"]}");
        assert_eq!(shown, "Chưa đồng ý.");
        assert_eq!(agree, Some(false));
        assert_eq!(disagreements, ["cache"]);
        let (shown, agree, _) = split_stance("Chỉ có chữ, không có JSON.");
        assert_eq!(shown, "Chỉ có chữ, không có JSON.");
        assert_eq!(agree, None);
    }

    #[test]
    fn convergence_needs_both_sides_from_round_two() {
        let agree = Turn {
            agree: Some(true),
            ..Default::default()
        };
        let round1 = Round {
            index: 1,
            codex: agree.clone(),
            claude: agree.clone(),
        };
        assert!(
            !round1.converged(),
            "round 1 proposals are independent, never 'agreement'"
        );
        let round2 = Round {
            index: 2,
            codex: agree.clone(),
            claude: agree.clone(),
        };
        assert!(round2.converged());
        let split = Round {
            index: 2,
            codex: agree,
            claude: Turn {
                agree: Some(false),
                ..Default::default()
            },
        };
        assert!(!split.converged());
    }

    #[test]
    fn verdict_is_parsed_and_unknown_choice_becomes_merged() {
        let verdict = parse_verdict(
            "Kết luận:\n{\"decision\":\"Dùng cách của Codex\",\"chosen\":\"???\",\"plan_steps\":[\"a\",\"b\"]}",
        )
        .unwrap();
        assert_eq!(verdict.decision, "Dùng cách của Codex");
        assert_eq!(verdict.chosen, "merged");
        assert_eq!(verdict.plan_steps, ["a", "b"]);
        assert!(parse_verdict("{\"decision\":\"\"}").is_none());
    }

    #[test]
    fn judge_prompt_contains_both_sides_and_stances() {
        let debate = Debate {
            request: "Sửa lỗi X".into(),
            rounds: vec![Round {
                index: 2,
                codex: Turn {
                    text: "Đề xuất A".into(),
                    agree: Some(false),
                    disagreements: vec!["khoá".into()],
                    ..Default::default()
                },
                claude: Turn {
                    text: "Đề xuất B".into(),
                    agree: Some(true),
                    ..Default::default()
                },
            }],
            ..Default::default()
        };
        let prompt = judge_prompt(&debate);
        assert!(prompt.contains("Sửa lỗi X"));
        assert!(prompt.contains("Đề xuất A") && prompt.contains("Đề xuất B"));
        assert!(prompt.contains("chưa đồng ý") && prompt.contains("khoá"));
        assert!(prompt.contains("KHÔNG tham gia"));
    }

    #[test]
    fn token_log_lines_are_summed() {
        assert_eq!(
            tokens_from_log("Token: vào 1200 (cache 300), ra 45 (suy luận 10)"),
            Some(1245)
        );
        assert_eq!(
            tokens_from_log("Token: vào 7, ra 3, chi phí $0.01"),
            Some(10)
        );
        assert_eq!(tokens_from_log("Đọc file a.rs"), None);
    }

    #[test]
    fn plan_request_carries_the_verdict_steps() {
        let debate = Debate {
            request: "Làm Y".into(),
            verdict: Some(Verdict {
                decision: "Gộp hai hướng".into(),
                plan_steps: vec!["Bước một".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let text = plan_request(&debate);
        assert!(text.starts_with("Làm Y"));
        assert!(text.contains("Gộp hai hướng") && text.contains("1. Bước một"));
    }
}
