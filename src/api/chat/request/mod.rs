//! Validation of an incoming chat request.

mod check;
mod folder;

pub(in crate::api::chat) use self::check::check_request;
pub(in crate::api::chat) use self::folder::resolve_folder;

#[cfg(test)]
use self::folder::plain_chat_folder;
#[cfg(test)]
use crate::chat::{self, ChatRequest};

#[cfg(test)]
mod tests;
