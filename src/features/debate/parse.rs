use super::types::Verdict;
use serde_json::Value;

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
