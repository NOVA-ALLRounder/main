use super::*;

pub fn iso_now_minus_hours(hours: f64) -> String {
    let seconds = (hours.max(0.0) * 3600.0).round() as i64;
    format_utc_ts(Utc::now() - Duration::seconds(seconds))
}

pub fn iso_now_minus_days(days: f64) -> String {
    let seconds = (days.max(0.0) * 86400.0).round() as i64;
    format_utc_ts(Utc::now() - Duration::seconds(seconds))
}

pub fn parse_iso_ts(value: &str) -> Option<DateTime<Utc>> {
    let normalized = value.trim().replace(' ', "T");
    chrono::DateTime::parse_from_rfc3339(&normalized)
        .ok()
        .map(|dt| dt.with_timezone(&Utc))
}

pub fn plus_one_microsecond_iso(value: &str) -> Option<String> {
    let ts = parse_iso_ts(value)?;
    Some(format_utc_ts(ts + Duration::microseconds(1)))
}

pub fn format_utc_ts(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}
