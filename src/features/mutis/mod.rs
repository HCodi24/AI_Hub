mod diagnosis;
mod event;
mod roles;
mod setting;
mod stats;

#[allow(unused_imports)]
pub use diagnosis::parse_diagnosis;
pub use diagnosis::{
    finish_diagnosis, list_diagnoses, normalize_error, start_diagnosis, DiagnosisError, DiagnosisRun,
};
pub use event::{record_plan_event, Event, EventLog, PlanEventError, PlanEventInput};
pub(crate) use roles::{resolve_lead, resolve_roles};
pub use setting::{apply_setting_change, replace_settings};
pub use stats::build_stats;

#[cfg(test)]
mod tests;
