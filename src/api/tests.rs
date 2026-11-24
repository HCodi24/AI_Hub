use super::local_host::request_allowed;
use axum::http::Method;

#[test]
fn validates_local_hosts() {
    assert!(request_allowed(
        7878,
        Some("127.0.0.1:7878"),
        &Method::GET,
        None,
    ));
    assert!(request_allowed(
        7878,
        Some("LOCALHOST:7878"),
        &Method::GET,
        None,
    ));
    assert!(!request_allowed(
        7878,
        Some("localhost:7880"),
        &Method::GET,
        None,
    ));
    assert!(!request_allowed(7878, None, &Method::GET, None));
}

#[test]
fn checks_origin_only_for_non_get_requests() {
    let host = Some("localhost:7878");
    let foreign = Some("http://example.com");
    assert!(request_allowed(7878, host, &Method::GET, foreign));
    assert!(!request_allowed(7878, host, &Method::POST, foreign));
    assert!(request_allowed(
        7878,
        host,
        &Method::POST,
        Some("HTTP://LOCALHOST:7878"),
    ));
    assert!(request_allowed(7878, host, &Method::POST, None));
    assert!(!request_allowed(7878, host, &Method::POST, Some("")));
}
