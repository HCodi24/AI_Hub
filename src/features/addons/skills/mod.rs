use anyhow::{Context, Result};
use ring::hmac;
use std::sync::{LazyLock, Mutex};

mod builtin;
mod copy;
mod install;
mod load;
mod shared;
mod swap;

pub(super) use builtin::builtin_skill_names;
pub(super) use copy::copy_checkout_dir;
pub use install::install_checkout_skill;
pub(crate) use load::load_skill;
pub(super) use shared::shared_skill_names;
pub(crate) use shared::{remove_shared_skill, shared_skill_catalogue, shared_skills_dir};
pub(super) use swap::swap_into_place;

pub enum SkillInstall {
    Installed,
    Conflict(String),
}

static SKILL_INSTALL_LOCK: Mutex<()> = Mutex::new(());
static CONFIRMATION_KEY: LazyLock<hmac::Key> =
    LazyLock::new(|| hmac::Key::new(hmac::HMAC_SHA256, uuid::Uuid::new_v4().as_bytes()));

pub(super) fn skill_install_lock() -> Result<std::sync::MutexGuard<'static, ()>> {
    SKILL_INSTALL_LOCK
        .lock()
        .ok()
        .context("Không mở được khóa cài skill")
}
