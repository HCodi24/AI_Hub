mod html;
mod listener;

pub(super) use listener::open;

use super::{ApiError, ApiResult};
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, uri::Authority, HeaderMap, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use reqwest::{Client, Url};
use std::{error::Error, io, sync::LazyLock, time::Duration};

const PICKER_PATH: &str = "/__mutis/picker.js";

static CLIENT: LazyLock<reqwest::Result<Client>> = LazyLock::new(|| {
    Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(2))
        .build()
});

async fn picker() -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        include_str!("../../../web/public/mutis-picker.js"),
    )
        .into_response()
}

async fn forward(State(port): State<u16>, request: Request) -> Response {
    if request.method() != Method::GET {
        return (StatusCode::METHOD_NOT_ALLOWED, [(header::ALLOW, "GET")]).into_response();
    }
    let path_and_query = request
        .uri()
        .path_and_query()
        .map_or("/", |value| value.as_str());
    let proxy_host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .filter(|host| !host.contains('@'))
        .and_then(|host| host.parse::<Authority>().ok());

    let Ok(client) = &*CLIENT else {
        return bad_gateway();
    };
    let mut headers = request.headers().clone();
    strip_hop_headers(&mut headers);
    headers.remove(header::HOST);
    headers.remove(header::CONTENT_LENGTH);
    headers.insert(header::ACCEPT_ENCODING, HeaderValue::from_static("identity"));

    let send = |host| {
        client
            .get(format!("http://{host}:{port}{path_and_query}"))
            .headers(headers.clone())
            .send()
    };
    let upstream = match send("127.0.0.1").await {
        Err(error) if connection_refused(&error) => send("[::1]").await,
        response => response,
    };
    let Ok(upstream) = upstream else {
        return bad_gateway();
    };

    let status = upstream.status();
    let mut headers = upstream.headers().clone();
    strip_hop_headers(&mut headers);
    headers.remove(header::X_FRAME_OPTIONS);
    headers.remove(header::CONTENT_SECURITY_POLICY);
    if status.is_redirection() {
        if let Some(location) = headers
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|location| rewrite_location(location, port, proxy_host.as_ref()?))
        {
            headers.insert(header::LOCATION, location);
        }
    }

    let body = if status != StatusCode::NOT_MODIFIED
        && status != StatusCode::NO_CONTENT
        && status != StatusCode::RESET_CONTENT
        && headers
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/html"))
            })
    {
        let Ok(bytes) = upstream.bytes().await else {
            return bad_gateway();
        };
        headers.remove(header::CONTENT_LENGTH);
        headers.remove(header::ETAG);
        Body::from(html::inject(&bytes))
    } else {
        Body::from_stream(upstream.bytes_stream())
    };
    let mut response = Response::new(body);
    *response.status_mut() = status;
    *response.headers_mut() = headers;
    response
}

fn invalid_port() -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "Cổng xem trước không hợp lệ")
}

fn validate_port(value: &str) -> ApiResult<u16> {
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid_port());
    }
    value
        .parse()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(invalid_port)
}

fn bad_gateway() -> Response {
    (StatusCode::BAD_GATEWAY, "Không thể kết nối máy chủ xem trước").into_response()
}

fn connection_refused(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(cause) = source {
        if cause
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::ConnectionRefused)
        {
            return true;
        }
        source = cause.source();
    }
    false
}

fn strip_hop_headers(headers: &mut HeaderMap) {
    let connection_headers: Vec<header::HeaderName> = headers
        .get_all(header::CONNECTION)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .filter_map(|name| name.trim().parse().ok())
        .collect();
    headers.remove(header::CONNECTION);
    for name in connection_headers {
        headers.remove(name);
    }
    for name in [
        "keep-alive",
        "proxy-authenticate",
        "proxy-authorization",
        "proxy-connection",
        "te",
        "trailer",
        "transfer-encoding",
        "upgrade",
    ] {
        headers.remove(name);
    }
}

fn rewrite_location(location: &str, port: u16, proxy_host: &Authority) -> Option<HeaderValue> {
    let url = Url::parse(location).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !matches!(url.host_str(), Some("127.0.0.1" | "[::1]" | "localhost"))
        || url.port_or_known_default() != Some(port)
    {
        return None;
    }
    let mut destination = format!("http://{proxy_host}{}", url.path());
    if let Some(query) = url.query() {
        destination.push('?');
        destination.push_str(query);
    }
    if let Some(fragment) = url.fragment() {
        destination.push('#');
        destination.push_str(fragment);
    }
    HeaderValue::from_str(&destination).ok()
}

#[cfg(test)]
mod tests;
