use super::list::sorted_entries;
use std::fs;

#[test]
fn sorts_directories_first_and_names_without_case() {
    let dir = std::env::temp_dir().join(format!("mutis-folder-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&dir).unwrap();
    for name in ["Zebra", "apple"] {
        fs::create_dir(dir.join(name)).unwrap();
    }
    for name in ["Bear.txt", "ant.txt"] {
        fs::write(dir.join(name), b"").unwrap();
    }

    let entries = sorted_entries(&dir.to_string_lossy());
    for name in ["Bear.txt", "ant.txt"] {
        fs::remove_file(dir.join(name)).unwrap();
    }
    for name in ["Zebra", "apple"] {
        fs::remove_dir(dir.join(name)).unwrap();
    }
    fs::remove_dir(&dir).unwrap();

    assert_eq!(
        entries.unwrap(),
        [
            (true, "apple".to_owned()),
            (true, "Zebra".to_owned()),
            (false, "ant.txt".to_owned()),
            (false, "Bear.txt".to_owned()),
        ],
    );
}

#[test]
fn rejects_a_missing_directory() {
    let missing = std::env::temp_dir().join(format!("mutis-folder-{}", uuid::Uuid::new_v4()));
    assert_eq!(
        sorted_entries(&missing.to_string_lossy()).unwrap_err().kind(),
        std::io::ErrorKind::NotFound,
    );
}
