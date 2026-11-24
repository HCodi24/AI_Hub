use super::*;

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("ai-hub-config-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn config_path(&self) -> PathBuf {
        self.0.join("config.json")
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn saved_config_round_trips() {
    let dir = TempDir::new();
    let path = dir.config_path();
    let mut config = load_or_seed(&path).unwrap();
    config.default_folder = "example-project".into();
    save(&path, &config).unwrap();

    let loaded = load_or_seed(&path).unwrap();
    assert_eq!(
        serde_json::to_value(&loaded).unwrap(),
        serde_json::to_value(&config).unwrap(),
    );
}

#[test]
fn save_leaves_only_the_config_file() {
    let dir = TempDir::new();
    save(&dir.config_path(), &Config::seeded()).unwrap();

    let entries: Vec<_> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries, vec![dir.config_path()]);
}

#[test]
fn missing_config_is_seeded_and_saved() {
    let dir = TempDir::new();
    let path = dir.config_path();
    assert!(!path.exists());

    let seeded = load_or_seed(&path).unwrap();
    assert!(!seeded.providers.is_empty());
    let saved: Config = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(saved).unwrap(),
        serde_json::to_value(seeded).unwrap(),
    );
}

#[test]
fn failed_rename_removes_the_temporary_file() {
    let dir = TempDir::new();
    let path = dir.config_path();
    fs::create_dir(&path).unwrap();

    assert!(save(&path, &Config::seeded()).is_err());
    let entries: Vec<_> = fs::read_dir(&dir.0)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    assert_eq!(entries, vec![path]);
}

#[test]
fn invalid_json_is_rejected_and_left_untouched() {
    let dir = TempDir::new();
    let path = dir.config_path();
    let invalid = b"{ invalid JSON";
    fs::write(&path, invalid).unwrap();

    let error = load_or_seed(&path).err().unwrap();
    assert_eq!(
        error.to_string(),
        format!("Could not parse config file at {}", path.display()),
    );
    assert_eq!(fs::read(&path).unwrap().as_slice(), invalid);
}
