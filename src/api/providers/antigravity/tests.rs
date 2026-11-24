use super::models::parse_models;

#[test]
fn parses_model_ids_from_the_first_column() {
    let stdout = "id\tName\nMODEL\tName\n\n  \n\
                  alpha\tAlpha Model\nalpha\tDuplicate\ninvalid id\tInvalid\n beta \tBeta Model\n";
    assert_eq!(parse_models(stdout), ["alpha", "beta"]);
}

#[test]
fn counts_only_listed_models() {
    assert_eq!(
        parse_models(
            "gemini-4-neon-high\tGemini 4 Neon (High)\n\n\
             gemini-4-carbon-low\tGemini 4 Carbon (Low)\n"
        )
        .len(),
        2,
    );
    assert_eq!(parse_models("  \n").len(), 0);
}
