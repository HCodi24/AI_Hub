use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn config_dir(config_path: &Path) -> Result<PathBuf> {
    config_path
        .parent()
        .map(Path::to_path_buf)
        .context("config path has no parent")
}

pub fn data_dir(config_path: &Path) -> Result<PathBuf> {
    config_dir(config_path)
}

pub fn chats_dir(config_path: &Path) -> Result<PathBuf> {
    Ok(data_dir(config_path)?.join("chats"))
}

pub fn debates_dir(config_path: &Path) -> Result<PathBuf> {
    Ok(data_dir(config_path)?.join("debates"))
}

pub fn plans_dir(config_path: &Path) -> Result<PathBuf> {
    Ok(data_dir(config_path)?.join("plans"))
}

pub fn mutis_dir(config_path: &Path) -> Result<PathBuf> {
    Ok(data_dir(config_path)?.join("mutis"))
}

pub fn backups_dir(config_path: &Path) -> Result<PathBuf> {
    Ok(data_dir(config_path)?.join("backups"))
}
