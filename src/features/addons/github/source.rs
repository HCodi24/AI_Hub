use anyhow::{bail, Result};

/// `owner/repo` or `https://github.com/owner/repo[.git]` -> `https://github.com/owner/repo`. Anything else is
/// refused: the link reaches `git clone` and the plugin CLIs, so only plain GitHub repositories are accepted.
pub fn github_source(link: &str) -> Result<String> {
    let link = link.trim().trim_end_matches('/');
    let path = link.strip_prefix("https://github.com/").unwrap_or(link);
    let path = path.strip_suffix(".git").unwrap_or(path);
    let parts: Vec<&str> = path.split('/').collect();
    let part_ok = |part: &&str| {
        !part.is_empty()
            && part.len() <= 100
            && !part.starts_with(['-', '.'])
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    };
    if parts.len() != 2 || !parts.iter().all(part_ok) {
        bail!("Chỉ nhận link GitHub dạng owner/repo hoặc https://github.com/owner/repo");
    }
    Ok(format!("https://github.com/{path}"))
}
