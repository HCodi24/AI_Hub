use crate::features::providers::{
    cli::antigravity::{remove_if_exists, signed_out as antigravity_signed_out, EMAIL_FILE},
    connection::ConnectionTest,
};
use std::{io, path::Path};

pub(crate) fn logout(home: &Path) -> io::Result<ConnectionTest> {
    // Signing out deletes the web account's token file from its own home.
    for file in [
        home.join(".gemini")
            .join("antigravity-cli")
            .join("antigravity-oauth-token"),
        home.join(EMAIL_FILE),
    ] {
        remove_if_exists(&file)?;
    }
    Ok(antigravity_signed_out())
}
