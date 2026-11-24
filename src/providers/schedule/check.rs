use crate::core::text::extract_json;
use crate::providers::delegate::Delegation;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    Unclear,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub severity: String,
    pub file: String,
    pub problem: String,
    pub fix: String,
}

impl Issue {
    /// High and medium findings make the worker redo the task; low ones only go to the coordinator.
    pub fn is_severe(&self) -> bool {
        matches!(self.severity.to_lowercase().as_str(), "high" | "medium" | "critical" | "blocker")
    }
}

#[derive(Clone, Debug)]
pub struct CheckResult {
    pub verdict: Verdict,
    pub issues: Vec<Issue>,
    pub notes: String,
    pub error: Option<String>,
}

impl CheckResult {
    pub(in crate::providers) fn unclear(why: &str) -> Self {
        Self { verdict: Verdict::Unclear, issues: Vec::new(), notes: String::new(), error: Some(why.to_owned()) }
    }
}

/// The checker's JSON answer; anything unreadable is "unclear" (reported, never turned into a fix task).
pub fn parse_check(text: &str) -> CheckResult {
    let Some(json) = extract_json(text) else { return CheckResult::unclear("the checker did not answer with JSON") };
    let Ok(value) = serde_json::from_str::<Value>(json) else { return CheckResult::unclear("the checker's JSON could not be read") };
    let verdict = match value["verdict"].as_str().map(|v| v.trim().to_uppercase()).as_deref() {
        Some("PASS") => Verdict::Pass,
        Some("FAIL") => Verdict::Fail,
        _ => Verdict::Unclear,
    };
    let text_of = |item: &Value, key: &str| item[key].as_str().unwrap_or_default().trim().to_owned();
    let issues = value["issues"].as_array().into_iter().flatten().map(|item| Issue {
        severity: if text_of(item, "severity").is_empty() { "medium".into() } else { text_of(item, "severity") },
        file: text_of(item, "file"),
        problem: text_of(item, "problem"),
        fix: text_of(item, "fix"),
    }).filter(|issue| !issue.problem.is_empty()).collect();
    CheckResult { verdict, issues, notes: text_of(&value, "notes"), error: None }
}

/// What the chat shows for one check, and what the coordinator reads. The headings are Vietnamese (the app's own prose);
/// the verdict token, the task title, the severity, the file paths and the checker's own text stay verbatim.
pub fn check_text(task: &Delegation, attempt: usize, check: &CheckResult) -> String {
    let verdict = match check.verdict {
        Verdict::Pass => "PASS",
        Verdict::Fail => "FAIL",
        Verdict::Unclear => "UNCLEAR",
    };
    let mut text = format!("Kiểm tra độc lập cho \"{}\" (lần {attempt}): {verdict}\n", task.title.trim());
    if let Some(error) = &check.error {
        text.push_str(&format!("Lỗi của checker: {error}\n"));
    }
    for issue in &check.issues {
        text.push_str(&format!("- [{}] {}: {} -> {}\n", issue.severity, issue.file, issue.problem, issue.fix));
    }
    if !check.notes.is_empty() {
        text.push_str(&format!("Ghi chú: {}\n", check.notes));
    }
    text
}

pub(super) const MAX_TASK_FILES: usize = 12;

/// File paths named in a checker issue's `file` field: several may be joined by `;` or `,`, and each may carry a
/// `:line` suffix or backticks. Anything with a space is not a path.
pub fn issue_files(raw: &str) -> Vec<String> {
    raw.split([';', ',']).map(|part| {
        let part = part.trim().trim_matches('`').trim_start_matches("./");
        let path = match part.rsplit_once(':') {
            Some((path, line)) if !line.is_empty() && line.chars().all(|c| c.is_ascii_digit() || c == '-') => path,
            _ => part,
        };
        path.trim().to_owned()
    }).filter(|path| !path.is_empty() && !path.contains(' ')).collect()
}

pub(super) fn same_path(a: &str, b: &str) -> bool {
    let clean = |p: &str| p.replace('\\', "/").trim_start_matches("./").to_lowercase();
    clean(a) == clean(b)
}

/// The same task again, with the severe findings added to its instructions. The title stays: the 4 attempts are shared.
pub(super) fn fix_task(task: &Delegation, severe: &[&Issue]) -> Delegation {
    let list = severe.iter().map(|i| format!("- {}: {} (fix: {})", i.file, i.problem, i.fix)).collect::<Vec<_>>().join("\n");
    Delegation {
        instructions: format!("{}\n\nAn independent check of your previous result found these problems. Fix exactly these and keep everything else:\n{list}", task.instructions.trim()),
        ..task.clone()
    }
}
