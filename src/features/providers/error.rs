#[derive(Debug)]
pub(crate) enum ProviderErrorKind {
    BadRequest,
    NotFound,
    BadGateway,
    GatewayTimeout,
    Internal,
}

#[derive(Debug)]
pub(crate) struct ProviderError {
    pub(crate) kind: ProviderErrorKind,
    pub(crate) message: String,
}

impl ProviderError {
    pub(crate) fn new(kind: ProviderErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

pub(crate) type ProviderResult<T> = Result<T, ProviderError>;
