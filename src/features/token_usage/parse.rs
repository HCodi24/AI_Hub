use super::JS_SAFE_MAX;

fn same_char_ci(left: char, right: char) -> bool {
    left == right || left.to_lowercase().eq(right.to_lowercase())
}

/// Case-insensitive prefix match that also folds non-ASCII letters (`VÀO` == `vào`).
fn strip_prefix_ci<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let mut offset = 0usize;
    for expected in prefix.chars() {
        let actual = text[offset..].chars().next()?;
        if !same_char_ci(actual, expected) {
            return None;
        }
        offset += actual.len_utf8();
    }
    Some(&text[offset..])
}

/// The `\s+` the frontend requires between a keyword and its number.
fn require_space(text: &str) -> Option<&str> {
    let trimmed = text.trim_start();
    (trimmed.len() < text.len()).then_some(trimmed)
}

/// `\d+` with overflow and safe-integer rejection.
fn digits(text: &str) -> Option<(u64, &str)> {
    let end = text
        .find(|character: char| !character.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 {
        return None;
    }
    let value: u64 = text[..end].parse().ok()?;
    (value <= JS_SAFE_MAX).then(|| (value, &text[end..]))
}

/// The optional `(...)` metadata group (cache, reasoning, ...): never added to the counts.
fn skip_paren(text: &str) -> &str {
    if let Some(inner) = text.strip_prefix('(') {
        if let Some(close) = inner.find(')') {
            return &inner[close + 1..];
        }
    }
    text
}

/// The optional `[\d.,]+` cost tail.
fn cost(text: &str) -> Option<&str> {
    let end = text
        .find(|character: char| !(character.is_ascii_digit() || character == '.' || character == ','))
        .unwrap_or(text.len());
    (end > 0).then(|| &text[end..])
}

/// Parse one log line exactly like `web/src/lib/chat-token-usage.ts`:
/// `Token: vào 1200 (cache 300), ra 45 (suy luận 10)` with an optional `, chi phí $0.12` tail.
/// The keywords are case-insensitive (including Vietnamese letters) and the whitespace after
/// `vào`/`ra` is required, exactly as in the frontend `REPORT` expression.
pub fn parse_report(log: &str) -> Option<(u64, u64)> {
    let rest = strip_prefix_ci(log.trim_start(), "token:")?;
    let rest = strip_prefix_ci(rest.trim_start(), "vào")?;
    let (input, rest) = digits(require_space(rest)?)?;
    let rest = skip_paren(rest.trim_start()).trim_start();
    let rest = rest.strip_prefix(',')?.trim_start();
    let rest = strip_prefix_ci(rest, "ra")?;
    let (output, rest) = digits(require_space(rest)?)?;
    let rest = skip_paren(rest.trim_start()).trim_start();
    let rest = if rest.is_empty() {
        rest
    } else {
        let rest = rest.strip_prefix(',')?.trim_start();
        let rest = strip_prefix_ci(rest, "chi phí")?.trim_start();
        let rest = rest.strip_prefix('$').unwrap_or(rest).trim_start();
        cost(rest)?.trim_start()
    };
    rest.is_empty().then_some((input, output))
}

/// True when this log line is an accepted token report, i.e. worth an observation.
pub fn is_token_report(log: &str) -> bool {
    parse_report(log).is_some()
}

/// The last valid report of one message, matching the summary the chat UI already shows.
pub fn last_report(logs: &[String]) -> Option<(u64, u64)> {
    logs.iter().rev().find_map(|log| parse_report(log))
}
