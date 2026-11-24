use super::super::paths::check_directories;
use anyhow::{bail, Context, Result};
use std::{fs, path::Path};

#[cfg(test)]
mod tests;

pub(in crate::features::addons) fn swap_into_place(
    skills_dir: &Path,
    destination: &Path,
    populate: impl FnOnce(&Path) -> Result<()>,
    previous_exists: impl FnOnce() -> Result<bool>,
) -> Result<()> {
    fs::create_dir_all(skills_dir)?;
    let stage = skills_dir.join(format!(".mutis-skill-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&stage)?;
    let staged = stage.join("new");
    let previous = stage.join("previous");
    let mut keep_backup = false;
    let result = (|| {
        populate(&staged)?;
        check_directories(skills_dir)?;
        let had_previous = previous_exists()?;
        if had_previous {
            fs::rename(destination, &previous).context("Không chuyển được bản skill hiện tại")?;
        }
        if let Err(error) = fs::rename(&staged, destination) {
            if had_previous {
                if let Err(restore) = fs::rename(&previous, destination) {
                    keep_backup = true;
                    bail!("Không cài được skill ({error}). Không khôi phục được bản cũ ({restore}). Bản cũ ở «{}»", previous.display());
                }
            }
            return Err(error).context("Không đưa được skill vào thư mục đích");
        }
        Ok(())
    })();
    if !keep_backup && check_directories(&stage).is_ok() {
        let _ = fs::remove_dir_all(&stage);
    }
    result
}
