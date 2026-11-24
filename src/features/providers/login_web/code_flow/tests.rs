use super::parse::{
    allowed_login_url, extract_https_url, logged_in_email, push_tail, validate_login_code,
    CLAUDE_DOMAINS, GOOGLE_DOMAINS,
};

#[test]
fn extracts_login_url_from_cli_intro_and_prompt_output() {
    let output = concat!(
        "Opening browser to sign in…\n",
        "If the browser didn't open, visit: https://claude.com/cai/oauth/authorize?code=true&client_id=abc",
        "&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&state=xyz\n",
        "Paste code here if prompted >"
    );
    let url = extract_https_url(output).unwrap();
    assert!(url.starts_with("https://claude.com/cai/oauth/authorize?"));
    assert!(allowed_login_url(url, CLAUDE_DOMAINS));
}

#[test]
fn antigravity_sign_in_allows_only_google_accounts_and_reads_the_email() {
    assert!(allowed_login_url("https://accounts.google.com/o/oauth2/auth?client_id=x", GOOGLE_DOMAINS));
    assert!(!allowed_login_url("https://claude.com/login", GOOGLE_DOMAINS));
    assert!(!allowed_login_url("https://accounts.google.com.evil.example/", GOOGLE_DOMAINS));
    let lines = vec!["Then paste the code".to_owned(), "Logged in as a.b@gmail.com.".to_owned()];
    assert_eq!(logged_in_email(&lines).as_deref(), Some("a.b@gmail.com"));
    assert_eq!(logged_in_email(&["Logged in as nobody".to_owned()]), None);
}

#[test]
fn allows_only_claude_and_anthropic_hosts() {
    for host in ["claude.com", "accounts.claude.ai", "platform.anthropic.com"] {
        assert!(allowed_login_url(&format!("https://{host}/oauth"), CLAUDE_DOMAINS));
    }
    for url in [
        "http://claude.com/login",
        "https://evilclaude.com/login",
        "https://example.com/?next=claude.com",
        "https://claude.ai:password@evil.example/login",
    ] {
        assert!(!allowed_login_url(url, CLAUDE_DOMAINS));
    }
}

#[test]
fn validates_confirmation_code_without_logging_it() {
    assert_eq!(validate_login_code("  abc123  ").unwrap(), "abc123");
    assert!(validate_login_code(&"x".repeat(513)).is_err());
    assert!(validate_login_code("abc\n123").is_err());
    assert!(validate_login_code("  \t ").is_err());
}

#[test]
fn push_tail_keeps_twenty_lines_and_truncates_long_lines() {
    let mut tail = Vec::new();
    for index in 0..20 {
        push_tail(&mut tail, &index.to_string());
    }
    let prefix = "é".repeat(2_000);
    push_tail(&mut tail, &format!("{prefix}x"));
    assert_eq!(tail.len(), 20);
    assert_eq!(tail[0], "1");
    assert_eq!(tail[19], format!("{prefix}…"));
}
