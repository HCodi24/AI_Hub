use crate::mutis::{self, Event};

pub(super) fn event_builder<'a>(
    plan_id: &'a Option<String>,
    task_id: &'a Option<String>,
    event_provider_id: &'a str,
) -> impl Fn(&str, Option<String>, bool, Option<u64>, Option<u64>, Option<String>) -> Event + 'a {
    move |kind: &str, task_kind, ok, duration_ms, tokens, detail| Event {
        ts: mutis::now(),
        plan_id: plan_id.clone(),
        task_id: task_id.clone(),
        provider_id: event_provider_id.to_owned(),
        kind: kind.to_owned(),
        task_kind,
        ok,
        duration_ms,
        tokens,
        detail,
    }
}
