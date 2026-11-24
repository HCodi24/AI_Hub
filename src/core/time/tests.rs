use super::{civil, now_ms, now_secs};

#[test]
fn seconds_and_milliseconds_agree() {
    assert!(now_secs().abs_diff(now_ms() / 1000) <= 1);
}

#[test]
fn civil_converts_unix_seconds_to_utc() {
    for (secs, expected) in [
        (0, (1970, 1, 1, 0, 0, 0)),
        (86_399, (1970, 1, 1, 23, 59, 59)),
        (951_782_400, (2000, 2, 29, 0, 0, 0)),
        (1_709_164_800, (2024, 2, 29, 0, 0, 0)),
    ] {
        let date = civil(secs);
        assert_eq!(
            (
                date.year,
                date.month,
                date.day,
                date.hour,
                date.minute,
                date.second
            ),
            expected
        );
    }
}
