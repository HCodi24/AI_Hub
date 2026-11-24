use super::swap_into_place;
use std::{fs, path::PathBuf};

struct Fixture {
    root: PathBuf,
    destination: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mutis-swap-{}", uuid::Uuid::new_v4()));
        let destination = root.join("example");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("SKILL.md"), "old").unwrap();
        Self { root, destination }
    }

    fn assert_contents(&self, expected: &str) {
        assert_eq!(
            fs::read_to_string(self.destination.join("SKILL.md")).unwrap(),
            expected
        );
        assert_eq!(fs::read_dir(&self.root).unwrap().count(), 1);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn swap_replaces_the_destination_and_removes_the_stage() {
    let fixture = Fixture::new();
    swap_into_place(
        &fixture.root,
        &fixture.destination,
        |staged| {
            fs::create_dir(staged)?;
            fs::write(staged.join("SKILL.md"), "new")?;
            Ok(())
        },
        || Ok(true),
    )
    .unwrap();
    fixture.assert_contents("new");
}

#[test]
fn failed_population_preserves_the_destination_and_removes_the_stage() {
    let fixture = Fixture::new();
    let error = swap_into_place(
        &fixture.root,
        &fixture.destination,
        |staged| {
            fs::create_dir(staged)?;
            fs::write(staged.join("SKILL.md"), "partial")?;
            anyhow::bail!("populate failed");
        },
        || panic!("previous copy check ran after population failed"),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "populate failed");
    fixture.assert_contents("old");
}
