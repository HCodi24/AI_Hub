pub(super) fn extract_https_url(text: &str) -> Option<&str> {
    let start = text.find("https://")?;
    let candidate = &text[start..];
    let end = candidate
        .find(|c: char| c.is_whitespace() || c == '>')
        .unwrap_or(candidate.len());
    Some(candidate[..end].trim_end_matches(['.', ',', ';', ')', ']', '"', '\'']))
}
pub(super) fn allowed_login_url(value: &str, domains: &[&str]) -> bool {
    reqwest::Url::parse(value).ok().is_some_and(|url| {
        url.scheme() == "https" && url.username().is_empty() && url.password().is_none()
            && url.host_str().is_some_and(|host| domains.iter()
                .any(|domain| host == *domain || host.ends_with(&format!(".{domain}"))))
    })
}
pub(super) const CLAUDE_DOMAINS: &[&str] = &["claude.com", "claude.ai", "anthropic.com"];
pub(super) const GOOGLE_DOMAINS: &[&str] = &["accounts.google.com"];
/// `Logged in as a@b.c.` (agy's last line after a web sign-in).
pub(super) fn logged_in_email(lines: &[String]) -> Option<String> {
    lines.iter().rev().find_map(|line| {
        let rest = line.trim().strip_prefix("Logged in as ")?;
        let email = rest.trim_end_matches('.').trim();
        email.contains('@').then(|| email.to_owned())
    })
}
pub(super) fn validate_login_code(code: &str) -> Result<String, &'static str> {
    if code.chars().count() > 512 {
        return Err("Mã xác nhận dài quá 512 ký tự");
    }
    if code.chars().any(char::is_control) {
        return Err("Mã xác nhận không được chứa ký tự điều khiển");
    }
    let trimmed = code.trim();
    if trimmed.is_empty() {
        return Err("Hãy nhập mã xác nhận");
    }
    Ok(trimmed.to_owned())
}

pub(super) fn push_tail(tail: &mut Vec<String>, line: &str) {
    if tail.len() == 20 {
        tail.remove(0);
    }
    tail.push(crate::core::text::truncate_chars(line, 2_000));
}
