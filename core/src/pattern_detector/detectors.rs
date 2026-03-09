use super::support::{extract_day_key, extract_timestamp_utc, PatternAggregate};
use super::{DetectedPattern, PatternDetector, PatternType};
use chrono::{DateTime, Datelike, Timelike, Utc};
use std::collections::HashMap;

impl PatternDetector {
    pub(crate) fn detect_app_sequences(&self, events: &[String]) -> Vec<DetectedPattern> {
        let mut sequences: HashMap<String, PatternAggregate> = HashMap::new();
        let mut app_history: Vec<String> = Vec::new();

        for event_str in events {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let payload = val.get("payload").or_else(|| val.get("data"));

                if event_type == "app_switch" || event_type == "system.open" {
                    let day_key = extract_day_key(&val);
                    let timestamp = extract_timestamp_utc(&val);
                    let app_name = if let Some(p) = payload {
                        p.get("app").and_then(|v| v.as_str()).unwrap_or("")
                    } else {
                        ""
                    };

                    if !app_name.is_empty() {
                        let app = app_name.to_string();
                        let key = format!("app:{app}");
                        sequences.entry(key.clone()).or_default().record(
                            event_str,
                            day_key.clone(),
                            timestamp.as_ref(),
                        );

                        if let Some(last_app) = app_history.last() {
                            if last_app != &app {
                                let pair_key = format!("flow:{last_app}->{app}");
                                sequences.entry(pair_key).or_default().record(
                                    event_str,
                                    day_key.clone(),
                                    timestamp.as_ref(),
                                );
                            }
                        }
                        app_history.push(app);
                    }
                }
            }
        }

        sequences
            .into_iter()
            .filter(|(_, aggregate)| aggregate.occurrences >= self.config.min_occurrences)
            .map(|(key, aggregate)| {
                let is_flow = key.starts_with("flow:");
                let description = if is_flow {
                    format!(
                        "Workflow Cycle: {}",
                        key.replace("flow:", "").replace("->", " → ")
                    )
                } else {
                    format!("Heavy usage: {}", key.replace("app:", ""))
                };

                let pattern_id = self.stable_pattern_id(&PatternType::AppSequence, &description);

                DetectedPattern {
                    pattern_id,
                    pattern_type: PatternType::AppSequence,
                    description,
                    occurrences: aggregate.occurrences,
                    distinct_days: aggregate.distinct_days(),
                    weekday_occurrences: aggregate.weekday_occurrences,
                    work_hour_occurrences: aggregate.work_hour_occurrences,
                    similarity_score: if is_flow { 0.95 } else { 0.8 },
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }

    pub(crate) fn detect_keyword_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
        let mut keywords: HashMap<String, PatternAggregate> = HashMap::new();

        for event_str in events {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                let day_key = extract_day_key(&val);
                let timestamp = extract_timestamp_utc(&val);
                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if event_type == "key_input"
                    || event_type == "ui.type"
                    || event_type == "keyboard.type"
                {
                    let text = val
                        .get("payload")
                        .or_else(|| val.get("data"))
                        .and_then(|d| d.get("text"))
                        .and_then(|t| t.as_str());

                    if let Some(t) = text {
                        for word in t.split_whitespace() {
                            if word.len() >= 3 {
                                keywords.entry(word.to_lowercase()).or_default().record(
                                    event_str,
                                    day_key.clone(),
                                    timestamp.as_ref(),
                                );
                            }
                        }
                    }
                }
            }
        }

        keywords
            .into_iter()
            .filter(|(_, aggregate)| aggregate.occurrences >= 5)
            .map(|(keyword, aggregate)| {
                let description = format!("Repeated keyword: '{keyword}'");
                let pattern_id = self.stable_pattern_id(&PatternType::KeywordRepeat, &description);
                DetectedPattern {
                    pattern_id,
                    pattern_type: PatternType::KeywordRepeat,
                    description,
                    occurrences: aggregate.occurrences,
                    distinct_days: aggregate.distinct_days(),
                    weekday_occurrences: aggregate.weekday_occurrences,
                    work_hour_occurrences: aggregate.work_hour_occurrences,
                    similarity_score: 0.85,
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }

    pub(crate) fn detect_file_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
        let mut file_ops: HashMap<String, PatternAggregate> = HashMap::new();

        for event_str in events {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                let day_key = extract_day_key(&val);
                let timestamp = extract_timestamp_utc(&val);
                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let path_opt = val
                    .get("payload")
                    .or_else(|| val.get("data"))
                    .and_then(|d| {
                        if d.is_string() {
                            d.as_str()
                        } else {
                            d.get("path")
                                .and_then(|p| p.as_str())
                                .or_else(|| d.as_str())
                        }
                    });

                if event_type == "file_created" || event_type == "file_modified" {
                    if let Some(path) = path_opt {
                        if let Some(ext) = std::path::Path::new(path)
                            .extension()
                            .and_then(|e| e.to_str())
                        {
                            let key = format!("ext:{ext}");
                            file_ops.entry(key).or_default().record(
                                event_str,
                                day_key.clone(),
                                timestamp.as_ref(),
                            );
                        }
                    }
                }
            }
        }

        file_ops
            .into_iter()
            .filter(|(_, aggregate)| aggregate.occurrences >= 3)
            .map(|(pattern, aggregate)| {
                let description = format!("File pattern: {}", pattern.replace("ext:", "."));
                let pattern_id = self.stable_pattern_id(&PatternType::FilePattern, &description);
                DetectedPattern {
                    pattern_id,
                    pattern_type: PatternType::FilePattern,
                    description,
                    occurrences: aggregate.occurrences,
                    distinct_days: aggregate.distinct_days(),
                    weekday_occurrences: aggregate.weekday_occurrences,
                    work_hour_occurrences: aggregate.work_hour_occurrences,
                    similarity_score: 0.85,
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }

    pub(crate) fn detect_time_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
        let mut time_map: HashMap<(String, u32, u32), PatternAggregate> = HashMap::new();

        for event_str in events {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                let timestamp_str = val
                    .get("ts")
                    .or_else(|| val.get("timestamp"))
                    .and_then(|v| v.as_str());
                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if event_type == "app_switch" || event_type == "system.open" {
                    let app_name = val
                        .get("payload")
                        .or_else(|| val.get("data"))
                        .and_then(|p| p.get("app"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if !app_name.is_empty() {
                        let Some(ts) = timestamp_str else {
                            continue;
                        };
                        if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
                            let dt_utc: DateTime<Utc> = dt.with_timezone(&Utc);
                            let weekday = dt_utc.weekday().num_days_from_monday();
                            let hour = dt_utc.hour();

                            let key = (app_name.to_string(), weekday, hour);
                            time_map.entry(key).or_default().record(
                                event_str,
                                Some(dt_utc.format("%F").to_string()),
                                Some(&dt_utc),
                            );
                        }
                    }
                }
            }
        }

        time_map
            .into_iter()
            .filter(|(_, aggregate)| aggregate.occurrences >= self.config.min_occurrences)
            .map(|((app, weekday, hour), aggregate)| {
                let day_str = match weekday {
                    0 => "Monday",
                    1 => "Tuesday",
                    2 => "Wednesday",
                    3 => "Thursday",
                    4 => "Friday",
                    5 => "Saturday",
                    6 => "Sunday",
                    _ => "Day",
                };

                let description =
                    format!("Weekly routine: Uses {app} on {day_str}s around {hour}:00");
                let pattern_id =
                    self.stable_pattern_id(&PatternType::TimeBasedAction, &description);

                DetectedPattern {
                    pattern_id,
                    pattern_type: PatternType::TimeBasedAction,
                    description,
                    occurrences: aggregate.occurrences,
                    distinct_days: aggregate.distinct_days(),
                    weekday_occurrences: aggregate.weekday_occurrences,
                    work_hour_occurrences: aggregate.work_hour_occurrences,
                    similarity_score: 0.8,
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }
}
