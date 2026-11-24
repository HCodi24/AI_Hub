use serde::Serialize;

#[derive(Clone, Serialize)]
pub(crate) struct ConnectionTest {
    pub(crate) ok: bool,
    pub(crate) status: &'static str,
    pub(crate) message: String,
    pub(crate) detail: String,
    /// The account's plan, for sources that report one (Antigravity).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) plan: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) email: Option<String>,
}

pub(crate) fn connection(ok: bool, status: &'static str, message: &str, detail: &str) -> ConnectionTest {
    ConnectionTest {
        ok,
        status,
        message: message.into(),
        detail: detail.into(),
        plan: None,
        email: None,
    }
}
