use super::turn::effort_arg;
use crate::chat::Effort;

#[test]
fn effort_maps_to_codex_argument() {
    for (effort, expected) in [
        (Effort::Default, None),
        (Effort::None, Some("none")),
        (Effort::Minimal, Some("minimal")),
        (Effort::Low, Some("low")),
        (Effort::Medium, Some("medium")),
        (Effort::High, Some("high")),
        (Effort::Xhigh, Some("xhigh")),
        (Effort::Max, Some("max")),
        (Effort::Ultra, Some("ultra")),
    ] {
        assert_eq!(effort_arg(effort), expected);
    }
}
