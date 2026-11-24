use crate::{
    features::providers::{cli::claude::provider_error as error, error::ProviderResult},
    providers::cli_support,
};
use std::path::Path;

pub(crate) fn logout(home: &Path) -> ProviderResult<()> {
    // Local sign-out must not revoke tokens used by the user's original CLI.
    let credentials = home.join(".credentials.json");
    if credentials.exists() {
        std::fs::remove_file(credentials).map_err(|e| error(e.into()))?;
    }
    cli_support::set_auth_origin(home, None);
    Ok(())
}
