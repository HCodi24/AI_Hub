use crate::{
    config::ProviderConfig,
    features::providers::{
        cli::antigravity::{
            probe::probe as probe_antigravity, remove_if_exists, signed_out as antigravity_signed_out,
        },
        connection::ConnectionTest,
    },
};
use std::{io, path::Path};

pub(crate) async fn connect_machine_account(
    provider: &ProviderConfig,
    home: &Path,
    marker: &Path,
) -> io::Result<ConnectionTest> {
    let result = probe_antigravity(provider, home).await;
    if result.status == "connected" {
        remove_if_exists(marker)?;
    }
    Ok(result)
}

pub(crate) fn logout(marker: &Path) -> io::Result<ConnectionTest> {
    // A machine source only remembers the sign-out.
    std::fs::write(marker, [])?;
    Ok(antigravity_signed_out())
}
