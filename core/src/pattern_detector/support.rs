use chrono::{DateTime, Datelike, Timelike, Utc};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Debug, Clone, Default)]
pub(crate) struct PatternAggregate {
    pub(crate) occurrences: u32,
    pub(crate) samples: Vec<String>,
    distinct_days: HashSet<String>,
    pub(crate) weekday_occurrences: u32,
    pub(crate) work_hour_occurrences: u32,
}

impl PatternAggregate {
    pub(crate) fn record(
        &mut self,
        event_str: &str,
        day_key: Option<String>,
        timestamp: Option<&DateTime<Utc>>,
    ) {
        self.occurrences += 1;
        if self.samples.len() < 3 {
            self.samples.push(event_str.to_string());
        }
        if let Some(day_key) = day_key.filter(|value| !value.trim().is_empty()) {
            self.distinct_days.insert(day_key);
        }
        if let Some(timestamp) = timestamp {
            if is_weekday_utc(timestamp) {
                self.weekday_occurrences += 1;
            }
            if is_work_hour_utc(timestamp) {
                self.work_hour_occurrences += 1;
            }
        }
    }

    pub(crate) fn distinct_days(&self) -> u32 {
        let count = self.distinct_days.len();
        if count == 0 {
            1
        } else {
            count as u32
        }
    }
}

pub(crate) fn extract_day_key(value: &Value) -> Option<String> {
    Some(extract_timestamp_utc(value)?.format("%F").to_string())
}

pub(crate) fn extract_timestamp_utc(value: &Value) -> Option<DateTime<Utc>> {
    let timestamp = value
        .get("ts")
        .or_else(|| value.get("timestamp"))
        .and_then(|v| v.as_str())?;
    let parsed = DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some(parsed.with_timezone(&Utc))
}

pub(crate) fn env_u32(key: &str, default_value: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default_value)
}

pub(crate) fn env_f64(key: &str, default_value: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<f64>().ok())
        .unwrap_or(default_value)
}

fn workday_start_hour() -> u32 {
    env_u32("ALLVIA_REC_WORKDAY_START_HOUR", 8).min(23)
}

fn workday_end_hour() -> u32 {
    env_u32("ALLVIA_REC_WORKDAY_END_HOUR", 19).min(23)
}

pub(crate) fn is_weekday_utc(timestamp: &DateTime<Utc>) -> bool {
    timestamp.weekday().num_days_from_monday() < 5
}

pub(crate) fn is_work_hour_utc(timestamp: &DateTime<Utc>) -> bool {
    if !is_weekday_utc(timestamp) {
        return false;
    }
    let hour = timestamp.hour();
    let start = workday_start_hour();
    let end = workday_end_hour();
    if start <= end {
        hour >= start && hour <= end
    } else {
        hour >= start || hour <= end
    }
}

pub(crate) fn allow_single_app_patterns() -> bool {
    std::env::var("ALLVIA_REC_ALLOW_SINGLE_APP_PATTERNS")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
