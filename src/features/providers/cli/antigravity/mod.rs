use crate::features::providers::connection::{connection, ConnectionTest};
use std::{io, path::Path as FsPath};

pub(crate) mod models;
pub(crate) mod probe;
#[cfg(test)]
mod tests;

/// The signed-in account's email, saved by the web sign-in.
/// agy prints "Logged in as <email>." and has no command that shows it later.
pub(crate) const EMAIL_FILE: &str = ".mutis-email";

pub(crate) fn signed_out() -> ConnectionTest {
    connection(
        false,
        "not_configured",
        "Antigravity CLI chưa đăng nhập trong MutiS",
        "",
    )
}

pub(crate) fn remove_if_exists(path: &FsPath) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
