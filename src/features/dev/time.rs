use crate::core::time::{civil, now_secs, Civil};

pub fn iso_now() -> String {
    let Civil {
        year,
        month,
        day,
        hour,
        minute,
        second,
    } = civil(now_secs());
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}
