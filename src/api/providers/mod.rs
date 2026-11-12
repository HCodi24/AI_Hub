//! Web API for AI sources (providers): editing, model lists, connection tests, Claude sign-in and CLI discovery.
mod cli;
mod codex;
mod connection;
mod crud;
mod login;
mod models;

pub(crate) use cli::{cli_status, discover_cli};
pub(crate) use codex::{codex_status, codex_action, codex_settings, CodexConnection};
pub(crate) use codex::reap_connections as reap_codex_connections;
pub(crate) use connection::{cached_provider_test, test_provider, ConnectionTest};
pub(crate) use crud::{create_provider, remove_provider, update_provider};
pub(crate) use login::{login_provider_cancel, login_provider_code, login_provider_start};
pub(crate) use models::{provider_limits, provider_models};
