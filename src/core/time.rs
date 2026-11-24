use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests;

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct Civil {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

/// Converts Unix seconds to a UTC date in the proleptic Gregorian calendar.
pub fn civil(secs: u64) -> Civil {
    let days = (secs / 86_400) as i64;
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let rem = (secs % 86_400) as u32;
    Civil {
        year: year + i64::from(month <= 2),
        month: month as u32,
        day: day as u32,
        hour: rem / 3600,
        minute: rem / 60 % 60,
        second: rem % 60,
    }
}
