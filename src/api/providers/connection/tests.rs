use super::{api_check::failed_status, connection};
use reqwest::StatusCode;

#[test]
fn maps_failed_http_statuses() {
    for (code, openai, message, detail) in [
        (401, true, "Sai API key", "HTTP 401 Unauthorized"),
        (403, false, "Sai API key", "HTTP 403 Forbidden"),
        (404, true, "Sai Base URL", "HTTP 404"),
        (404, false, "Không kết nối được", "HTTP 404 Not Found"),
        (500, true, "Không kết nối được", "HTTP 500 Internal Server Error"),
    ] {
        let result = failed_status(StatusCode::from_u16(code).unwrap(), openai);
        assert!(!result.ok);
        assert_eq!(result.status, "error");
        assert_eq!(result.message, message);
        assert_eq!(result.detail, detail);
    }
}

#[test]
fn connection_sets_fields_and_leaves_account_information_empty() {
    let result = connection(true, "connected", "message", "detail");
    assert!(result.ok);
    assert_eq!(result.status, "connected");
    assert_eq!(result.message, "message");
    assert_eq!(result.detail, "detail");
    assert_eq!(result.plan, None);
    assert_eq!(result.email, None);
}
