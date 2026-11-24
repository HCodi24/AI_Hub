use super::{Config, CODEX_CLI_MODELS};

#[test]
fn default_folder_is_empty_for_seeded_and_deserialized_configs() {
    let config = Config::seeded();
    assert!(config.default_folder.is_empty());

    let mut json = serde_json::to_value(config).unwrap();
    json.as_object_mut().unwrap().remove("default_folder");
    let config: Config = serde_json::from_value(json).unwrap();
    assert!(config.default_folder.is_empty());
}

#[test]
fn seeded_codex_cli_contains_all_suggested_models_in_order() {
    let config = Config::seeded();
    let codex = config
        .providers
        .iter()
        .find(|provider| provider.kind == "codex_cli")
        .unwrap();
    assert_eq!(
        codex.models.iter().map(String::as_str).collect::<Vec<_>>(),
        CODEX_CLI_MODELS,
    );
}
