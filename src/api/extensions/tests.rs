use super::action::events;
use axum::response::sse::Event;

#[test]
fn ansi_sequences_are_removed() {
    assert_eq!(
        crate::core::text::strip_ansi("\x1b[31mred\x1b[0m"),
        "red"
    );
}
#[test]
fn backup_timestamp_has_required_shape() {
    let stamp = super::backup::backup_timestamp();
    assert_eq!(stamp.len(), 15);
    assert_eq!(stamp.as_bytes()[8], b'-');
}

#[test]
fn maps_action_output_to_sse_events() {
    for (input, expected) in [
        (Ok("line".into()), vec![("log", r#""line""#)]),
        (Err(String::new()), vec![("done", r#"{"ok":true}"#)]),
        (
            Err("failed".into()),
            vec![
                ("error", r#"{"message":"failed"}"#),
                ("done", r#"{"ok":false}"#),
            ],
        ),
    ] {
        let expected = expected
            .into_iter()
            .map(|(name, data)| Event::default().event(name).data(data))
            .collect::<Vec<_>>();
        assert_eq!(format!("{:?}", events(input)), format!("{expected:?}"));
    }
}
