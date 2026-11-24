use super::origin_ok;

#[test]
fn same_origin_and_host_are_allowed() {
    assert!(origin_ok(Some("localhost:3000"), Some("localhost:3000"), false));
}

#[test]
fn different_origin_is_denied() {
    assert!(!origin_ok(Some("example.com"), Some("localhost:3000"), false));
}

#[test]
fn missing_origin_is_denied() {
    assert!(!origin_ok(None, None, true));
}

#[test]
fn dev_web_origin_requires_source_checkout() {
    for origin in ["127.0.0.1:7880", "localhost:7880"] {
        assert!(origin_ok(Some(origin), Some("localhost:3000"), true));
        assert!(!origin_ok(Some(origin), Some("localhost:3000"), false));
    }
}

#[test]
fn https_origin_is_denied() {
    let origin = "https://localhost:3000".strip_prefix("http://");
    assert!(!origin_ok(origin, Some("localhost:3000"), true));
}
