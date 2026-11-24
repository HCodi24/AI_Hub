use serde_json::Value;
use std::path::Path;

/// Account shown for a signed-in CLI source; never contains credentials.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AuthAccount {
    pub logged_in: bool,
    pub email: Option<String>,
    pub plan: Option<String>,
}

/// `claude auth status` JSON: `loggedIn` plus the optional `email` and `subscriptionType`.
pub fn parse_auth_account(output: &[u8]) -> Option<AuthAccount> {
    let value: Value = serde_json::from_slice(output).ok()?;
    let text = |key: &str| {
        value[key]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(AuthAccount {
        logged_in: value["loggedIn"].as_bool()?,
        email: text("email"),
        plan: text("subscriptionType"),
    })
}

/// Marker in a MutiS source home telling where its sign-in came from: `cli` (copied from this machine) or `web`.
const AUTH_ORIGIN_FILE: &str = ".mutis-origin";

pub fn set_auth_origin(home: &Path, origin: Option<&str>) {
    let path = home.join(AUTH_ORIGIN_FILE);
    let _ = match origin {
        Some(origin) => std::fs::write(path, origin),
        None => std::fs::remove_file(path),
    };
}

/// Short status for a signed-in source, shown under its name: "Đã kết nối · a@b.c · lấy từ máy".
pub fn connected_message(email: Option<&str>, origin: Option<&str>) -> String {
    let mut message = String::from("Đã kết nối");
    if let Some(email) = email {
        message.push_str(" · ");
        message.push_str(email);
    }
    match origin {
        Some("cli") => message.push_str(" · lấy từ máy"),
        Some("web") => message.push_str(" · đăng nhập web"),
        _ => {}
    }
    message
}

pub fn auth_origin(home: &Path) -> Option<String> {
    let text = std::fs::read_to_string(home.join(AUTH_ORIGIN_FILE)).ok()?;
    let text = text.trim();
    matches!(text, "cli" | "web").then(|| text.to_owned())
}
