use crate::config::ProviderConfig;

/// CLI sources sign in one way only: machine sources reuse the installed CLI's account, web sources use the web.
pub(crate) fn ensure_login_mode(
    provider: &ProviderConfig,
    web: bool,
) -> crate::features::providers::error::ProviderResult<()> {
    use crate::features::providers::error::{ProviderError, ProviderErrorKind};
    if provider.web_login == web {
        return Ok(());
    }
    Err(ProviderError::new(
        ProviderErrorKind::BadRequest,
        if provider.web_login {
            "Nguồn này đăng nhập qua web, không dùng tài khoản CLI trên máy"
        } else {
            "Nguồn này dùng tài khoản CLI trên máy; muốn đăng nhập web hãy thêm ở tab Tài khoản web"
        },
    ))
}
