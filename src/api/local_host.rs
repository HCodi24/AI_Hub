use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};

pub(super) async fn enforce_local_host(
    State(port): State<u16>,
    request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get("host")
        .and_then(|value| value.to_str().ok());
    let origin = request
        .headers()
        .get("origin")
        .map(|value| value.to_str().unwrap_or_default());
    if !request_allowed(port, host, request.method(), origin) {
        return (StatusCode::FORBIDDEN, "Forbidden").into_response();
    }
    next.run(request).await
}

pub(super) fn request_allowed(
    port: u16,
    host: Option<&str>,
    method: &Method,
    origin: Option<&str>,
) -> bool {
    let ipv4 = format!("127.0.0.1:{port}");
    let localhost = format!("localhost:{port}");
    if !host.is_some_and(|host| host == ipv4 || host.eq_ignore_ascii_case(&localhost)) {
        return false;
    }
    method == Method::GET
        || origin.is_none_or(|origin| {
            origin.eq_ignore_ascii_case(&format!("http://{ipv4}"))
                || origin.eq_ignore_ascii_case(&format!("http://{localhost}"))
        })
}
