pub mod addons;
pub mod attachments;
pub mod chat;
pub mod config;
pub mod debate;
pub mod dev;
pub mod extensions;
pub mod folder;
pub mod mcp;
pub mod memory;
pub mod mutis;
pub mod pets;
pub mod providers;
pub mod static_ui;
pub mod terminal;
pub mod verify;

mod error;
mod local_host;
mod router;

pub(crate) use error::{ApiError, ApiResult};
pub use router::router;

#[cfg(test)]
mod tests;
