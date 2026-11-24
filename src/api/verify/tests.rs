use super::{
    commands::{set_command, MAX_PROJECT_COMMANDS},
    folder::project_folder,
};
use std::collections::BTreeMap;

#[test]
fn project_folder_requires_an_existing_directory() {
    let dir = std::env::temp_dir().join(format!("mutis-verify-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&dir).unwrap();
    let file = dir.join("file.txt");
    std::fs::File::create(&file).unwrap();

    let expected = dir.canonicalize().map(crate::core::text::strip_verbatim);
    let found = project_folder(&dir.to_string_lossy());
    let missing = project_folder(&dir.join("missing").to_string_lossy());
    let not_directory = project_folder(&file.to_string_lossy());

    std::fs::remove_file(&file).unwrap();
    std::fs::remove_dir(&dir).unwrap();

    assert_eq!(found.ok(), Some(expected.unwrap()));
    assert_eq!(missing.unwrap_err().message, "Thư mục không tồn tại");
    assert_eq!(not_directory.unwrap_err().message, "Đường dẫn không phải thư mục");
}

#[test]
fn set_command_removes_an_empty_command() {
    let mut commands = BTreeMap::from([("project".into(), "cargo check".into())]);
    assert_eq!(set_command(&mut commands, "project".into(), String::new()), Ok(()));
    assert!(commands.is_empty());
}

#[test]
fn set_command_can_replace_but_cannot_add_at_the_limit() {
    let mut commands: BTreeMap<_, _> = (0..MAX_PROJECT_COMMANDS)
        .map(|index| (index.to_string(), "cargo check".to_owned()))
        .collect();
    assert_eq!(set_command(&mut commands, "0".into(), "cargo fmt".into()), Ok(()));
    assert_eq!(commands["0"], "cargo fmt");
    assert_eq!(
        set_command(&mut commands, "new".into(), "cargo check".into()),
        Err("Đã lưu quá nhiều dự án; xóa bớt trước"),
    );
    assert_eq!(commands.len(), MAX_PROJECT_COMMANDS);
    assert!(!commands.contains_key("new"));
}
