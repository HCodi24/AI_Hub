use super::validate::{clean_models, valid_provider_id, validate_provider};
use super::ProviderInput;

fn valid_input() -> ProviderInput {
    serde_json::from_value(serde_json::json!({
        "name": "Example",
        "kind": "openai_compat",
        "base_url": "https://example.com"
    }))
    .unwrap()
}

#[test]
fn provider_validation_rejects_invalid_fields() {
    assert!(validate_provider("valid-id", &valid_input()).is_ok());
    assert!(validate_provider("bad id", &valid_input()).is_err());

    let mut input = valid_input();
    input.name = " ".into();
    assert!(validate_provider("valid-id", &input).is_err());

    let mut input = valid_input();
    input.kind = "unknown".into();
    assert!(validate_provider("valid-id", &input).is_err());

    let mut input = valid_input();
    input.base_url.clear();
    assert!(validate_provider("valid-id", &input).is_err());
}

#[test]
fn provider_id_requires_one_to_eighty_allowed_characters() {
    assert!(!valid_provider_id(""));
    assert!(!valid_provider_id(&"a".repeat(81)));
    assert!(!valid_provider_id("with space"));
    assert!(valid_provider_id(&"a".repeat(80)));
    assert!(valid_provider_id("Provider_1-a"));
}

#[test]
fn model_cleaning_trims_names_and_drops_blanks() {
    let models = [" model-a ", "", "  ", "model-b"].map(str::to_owned);
    assert_eq!(clean_models(models.to_vec()), vec!["model-a", "model-b"]);
}
