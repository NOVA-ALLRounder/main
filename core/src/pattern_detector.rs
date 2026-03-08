#![allow(dead_code)]
use crate::db;
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

/// Detected pattern from user behavior logs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub pattern_id: String,
    pub pattern_type: PatternType,
    pub description: String,
    pub occurrences: u32,
    #[serde(default = "default_distinct_days")]
    pub distinct_days: u32,
    #[serde(default)]
    pub weekday_occurrences: u32,
    #[serde(default)]
    pub work_hour_occurrences: u32,
    pub similarity_score: f64,
    pub sample_events: Vec<String>,
    pub detected_at: DateTime<Utc>,
}

fn default_distinct_days() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    AppSequence,     // 앱 전환 시퀀스 (예: Chrome → Slack → Gmail)
    KeywordRepeat,   // 반복 키워드 입력
    FilePattern,     // 파일 작업 패턴
    TimeBasedAction, // 시간 기반 반복 작업
}

impl PatternType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppSequence => "app_sequence",
            Self::KeywordRepeat => "keyword_repeat",
            Self::FilePattern => "file_pattern",
            Self::TimeBasedAction => "time_based",
        }
    }
}

/// Pattern detection configuration
pub struct PatternConfig {
    pub min_occurrences: u32, // 최소 반복 횟수 (기본: 3)
    pub min_similarity: f64,  // 최소 유사도 (기본: 0.8)
    pub lookback_days: i64,   // 분석 기간 (기본: 7일)
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_occurrences: 3,
            min_similarity: 0.8,
            lookback_days: 7,
        }
    }
}

use crate::llm_gateway::LLMClient;

/// Pattern detector engine
pub struct PatternDetector {
    config: PatternConfig,
    llm_client: Option<std::sync::Arc<dyn LLMClient>>,
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PatternDetector {
    pub fn new() -> Self {
        let llm_client = crate::llm_gateway::OpenAILLMClient::new()
            .ok()
            .map(|c| std::sync::Arc::new(c) as std::sync::Arc<dyn LLMClient>);
        Self {
            config: PatternConfig::default(),
            llm_client,
        }
    }

    pub fn with_config(config: PatternConfig) -> Self {
        let llm_client = crate::llm_gateway::OpenAILLMClient::new()
            .ok()
            .map(|c| std::sync::Arc::new(c) as std::sync::Arc<dyn LLMClient>);
        Self { config, llm_client }
    }

    /// Analyze logs and detect patterns (uses DB)
    pub fn analyze(&self) -> Vec<DetectedPattern> {
        // Get recent events from DB
        let hours = self.config.lookback_days * 24;
        let events = match db::get_recent_events(hours) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        self.analyze_with_events(&events)
    }

    /// Analyze provided events (for testing or custom sources)
    pub fn analyze_with_events(&self, events: &[String]) -> Vec<DetectedPattern> {
        if events.is_empty() {
            return Vec::new();
        }

        let mut patterns = Vec::new();

        // Detect different pattern types
        patterns.extend(self.detect_app_sequences(events));
        patterns.extend(self.detect_keyword_patterns(events));
        patterns.extend(self.detect_file_patterns(events));
        patterns.extend(self.detect_time_patterns(events)); // New Logic

        // Filter by configuration thresholds
        patterns.retain(|p| {
            p.occurrences >= self.config.min_occurrences
                && p.similarity_score >= self.config.min_similarity
        });

        patterns
    }

    fn stable_pattern_id(&self, pattern_type: &PatternType, description: &str) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        pattern_type.as_str().hash(&mut hasher);
        description.to_lowercase().hash(&mut hasher);
        format!("p_{:x}", hasher.finish())
    }

    /// Detect app switching sequences
    fn detect_app_sequences(&self, events: &[String]) -> Vec<DetectedPattern> {
        let mut sequences: HashMap<String, PatternAggregate> = HashMap::new();
        let mut app_history: Vec<String> = Vec::new();

        for event_str in events {
            // Try parsing as Value to handle both new and old schemas flexibly
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                // Check for "event_type" (New) or "type" (Old)
                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Check for "payload" (New) or "data" (Old)
                // Note: payload/data might be an object
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
                        // 1. Single App Repeats
                        let key = format!("app:{}", app);
                        sequences.entry(key.clone()).or_default().record(
                            event_str,
                            day_key.clone(),
                            timestamp.as_ref(),
                        );

                        // 2. Interleaved Sequences (Bigrams)
                        if let Some(last_app) = app_history.last() {
                            if last_app != &app {
                                let pair_key = format!("flow:{}->{}", last_app, app);
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
                    similarity_score: if is_flow { 0.95 } else { 0.8 }, // Higher score for flows
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }

    /// Detect repeated keyword/text input patterns
    fn detect_keyword_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
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
                                let key = word.to_lowercase();
                                keywords.entry(key).or_default().record(
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
            .filter(|(_, aggregate)| aggregate.occurrences >= 5) // Keywords need more occurrences
            .map(|(keyword, aggregate)| {
                let description = format!("Repeated keyword: '{}'", keyword);
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

    /// Detect file operation patterns
    fn detect_file_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
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

                // Support simple string data or object payload
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
                            let key = format!("ext:{}", ext);
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

    /// Detect time-based patterns (e.g., "Always open Slack on Mon 9AM")
    fn detect_time_patterns(&self, events: &[String]) -> Vec<DetectedPattern> {
        // Key: (App, Weekday, Hour) -> (Count, Sample Events)
        // Weekday is 0-6 (Mon-Sun), Hour is 0-23
        let mut time_map: HashMap<(String, u32, u32), PatternAggregate> = HashMap::new();

        for event_str in events {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(event_str) {
                // 1. Extract Metadata (Time & Type)
                let timestamp_str = val
                    .get("ts")
                    .or_else(|| val.get("timestamp"))
                    .and_then(|v| v.as_str());

                let event_type = val
                    .get("event_type")
                    .or_else(|| val.get("type"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // 2. Filter for App Switches
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
                            let weekday = dt_utc.weekday().num_days_from_monday(); // 0=Mon
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

        // 3. Generate Patterns
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

                let description = format!(
                    "Weekly routine: Uses {} on {}s around {}:00",
                    app, day_str, hour
                );
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
                    similarity_score: 0.8, // Base score for time patterns
                    sample_events: aggregate.samples,
                    detected_at: Utc::now(),
                }
            })
            .collect()
    }

    /// Check if a pattern should generate a recommendation
    pub fn cosine_similarity(v1: &[f32], v2: &[f32]) -> f32 {
        if v1.len() != v2.len() {
            return 0.0;
        }
        let dot_product: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let norm_a: f32 = v1.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = v2.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot_product / (norm_a * norm_b)
    }

    /// Asynchronous analysis with Vector Memory (Fuzzy Matching)
    pub async fn analyze_async(&self) -> Vec<DetectedPattern> {
        let hours = self.config.lookback_days * 24;
        let events = match db::get_recent_events(hours) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        // 1. First get standard string-based patterns
        let patterns = self.analyze_with_events(&events);

        // 2. Enhance with semantic clustering if LLM is available
        if self.llm_client.is_some() {
            return self.merge_similar_patterns(patterns).await;
        }

        patterns
    }

    /// Merge patterns that are semantically similar (e.g. "Open Chrome" vs "Launch Chrome")
    pub async fn merge_similar_patterns(
        &self,
        patterns: Vec<DetectedPattern>,
    ) -> Vec<DetectedPattern> {
        if self.llm_client.is_none() {
            return patterns;
        }
        let client = self.llm_client.as_ref().unwrap();

        let mut final_patterns = Vec::new();
        let mut handled_indices = std::collections::HashSet::new();

        // Pre-compute embeddings for all patterns descriptions
        let mut embeddings: Vec<Option<Vec<f32>>> = Vec::new();
        for p in &patterns {
            if let Ok(emb) = client.get_embedding(&p.description).await {
                embeddings.push(Some(emb));
            } else {
                embeddings.push(None);
            }
        }

        for i in 0..patterns.len() {
            if handled_indices.contains(&i) {
                continue;
            }
            let mut current_group = patterns[i].clone();
            handled_indices.insert(i);

            if let Some(emb_i) = &embeddings[i] {
                for j in (i + 1)..patterns.len() {
                    if handled_indices.contains(&j) {
                        continue;
                    }

                    if let Some(emb_j) = &embeddings[j] {
                        let sim = Self::cosine_similarity(emb_i, emb_j);
                        if sim > 0.92 {
                            // High similarity threshold
                            // Merge j into i
                            current_group.occurrences += patterns[j].occurrences;
                            current_group.distinct_days =
                                current_group.distinct_days.max(patterns[j].distinct_days);
                            current_group
                                .sample_events
                                .extend(patterns[j].sample_events.clone());
                            // Keep description of the most frequent one
                            handled_indices.insert(j);
                        }
                    }
                }
            }
            final_patterns.push(current_group);
        }

        final_patterns
    }

    pub fn should_recommend(&self, pattern: &DetectedPattern) -> bool {
        let (min_occ, min_sim, min_days) = match pattern.pattern_type {
            PatternType::AppSequence => (
                env_u32(
                    "REC_MIN_OCCURRENCES_APP",
                    self.config.min_occurrences.max(4),
                ),
                env_f64(
                    "REC_MIN_SIMILARITY_APP",
                    self.config.min_similarity.max(0.8),
                ),
                env_u32("REC_MIN_DISTINCT_DAYS_APP", 2),
            ),
            PatternType::KeywordRepeat => (
                env_u32("REC_MIN_OCCURRENCES_KEYWORD", 5),
                env_f64(
                    "REC_MIN_SIMILARITY_KEYWORD",
                    self.config.min_similarity.max(0.85),
                ),
                env_u32("REC_MIN_DISTINCT_DAYS_KEYWORD", 2),
            ),
            PatternType::FilePattern => (
                env_u32("REC_MIN_OCCURRENCES_FILE", 4),
                env_f64(
                    "REC_MIN_SIMILARITY_FILE",
                    self.config.min_similarity.max(0.85),
                ),
                env_u32("REC_MIN_DISTINCT_DAYS_FILE", 2),
            ),
            PatternType::TimeBasedAction => (
                env_u32("REC_MIN_OCCURRENCES_TIME", 4),
                env_f64(
                    "REC_MIN_SIMILARITY_TIME",
                    self.config.min_similarity.max(0.8),
                ),
                env_u32("REC_MIN_DISTINCT_DAYS_TIME", 3),
            ),
        };

        if matches!(pattern.pattern_type, PatternType::AppSequence)
            && !allow_single_app_patterns()
            && pattern.description.starts_with("Heavy usage:")
        {
            return false;
        }

        pattern.occurrences >= min_occ
            && pattern.similarity_score >= min_sim
            && pattern.distinct_days >= min_days.max(1)
    }
}

#[derive(Debug, Clone, Default)]
struct PatternAggregate {
    occurrences: u32,
    samples: Vec<String>,
    distinct_days: HashSet<String>,
    weekday_occurrences: u32,
    work_hour_occurrences: u32,
}

impl PatternAggregate {
    fn record(
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

    fn distinct_days(&self) -> u32 {
        let count = self.distinct_days.len();
        if count == 0 {
            1
        } else {
            count as u32
        }
    }
}

fn extract_day_key(value: &serde_json::Value) -> Option<String> {
    Some(extract_timestamp_utc(value)?.format("%F").to_string())
}

fn extract_timestamp_utc(value: &serde_json::Value) -> Option<DateTime<Utc>> {
    let timestamp = value
        .get("ts")
        .or_else(|| value.get("timestamp"))
        .and_then(|v| v.as_str())?;
    let parsed = DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some(parsed.with_timezone(&Utc))
}

fn env_u32(key: &str, default_value: u32) -> u32 {
    std::env::var(key)
        .ok()
        .and_then(|value| value.trim().parse::<u32>().ok())
        .unwrap_or(default_value)
}

fn env_f64(key: &str, default_value: f64) -> f64 {
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

fn is_weekday_utc(timestamp: &DateTime<Utc>) -> bool {
    timestamp.weekday().num_days_from_monday() < 5
}

fn is_work_hour_utc(timestamp: &DateTime<Utc>) -> bool {
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

fn allow_single_app_patterns() -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_pattern_config_defaults() {
        let config = PatternConfig::default();
        assert_eq!(config.min_occurrences, 3);
        assert_eq!(config.min_similarity, 0.8);
    }

    #[test]
    fn test_app_sequence_detection() {
        let detector = PatternDetector::new();
        let events = vec![
            json!({"type": "app_switch", "data": {"app": "Slack"}}).to_string(),
            json!({"type": "app_switch", "data": {"app": "Slack"}}).to_string(),
            json!({"type": "app_switch", "data": {"app": "Slack"}}).to_string(),
            json!({"type": "app_switch", "data": {"app": "Chrome"}}).to_string(),
        ];

        let patterns = detector.analyze_with_events(&events);

        assert_eq!(patterns.len(), 1);
        let p = &patterns[0];
        assert_eq!(p.pattern_type, PatternType::AppSequence);
        assert!(p.description.contains("Slack"));
        assert_eq!(p.occurrences, 3);
        assert_eq!(p.distinct_days, 1);
    }

    #[test]
    fn test_keyword_pattern_detection() {
        let detector = PatternDetector::new();
        // Need 5 occurrences for keywords
        let events = vec![
            json!({"type": "key_input", "data": {"text": "invoice"}}).to_string(),
            json!({"type": "key_input", "data": {"text": "check invoice"}}).to_string(),
            json!({"type": "key_input", "data": {"text": "invoice list"}}).to_string(),
            json!({"type": "key_input", "data": {"text": "send invoice"}}).to_string(),
            json!({"type": "key_input", "data": {"text": "invoice"}}).to_string(),
        ];

        let patterns = detector.analyze_with_events(&events);

        assert!(!patterns.is_empty());
        let p = patterns
            .iter()
            .find(|p| p.description.contains("invoice"))
            .expect("Pattern not found in test");
        assert_eq!(p.occurrences, 5);
        assert_eq!(p.distinct_days, 1);
        assert_eq!(p.pattern_type, PatternType::KeywordRepeat);
    }

    #[test]
    fn test_file_pattern_detection() {
        let detector = PatternDetector::new();
        let events = vec![
            json!({"type": "file_created", "data": "/Downloads/report1.pdf"}).to_string(),
            json!({"type": "file_created", "data": "/Downloads/report2.pdf"}).to_string(),
            json!({"type": "file_created", "data": "/Downloads/final.pdf"}).to_string(),
        ];

        let patterns = detector.analyze_with_events(&events);

        assert_eq!(patterns.len(), 1);
        let p = &patterns[0];
        assert_eq!(p.pattern_type, PatternType::FilePattern);
        assert!(p.description.contains(".pdf"));
        assert_eq!(p.distinct_days, 1);
    }

    #[test]
    fn test_time_pattern_detection() {
        let detector = PatternDetector::new();
        // Simulate 3 events on Monday (Day 0) at 9:00 AM
        let events = vec![
            json!({"type": "app_switch", "ts": "2023-10-02T09:00:00Z", "data": {"app": "Slack"}})
                .to_string(), // Mon
            json!({"type": "app_switch", "ts": "2023-10-09T09:15:00Z", "data": {"app": "Slack"}})
                .to_string(), // Mon
            json!({"type": "app_switch", "ts": "2023-10-16T09:05:00Z", "data": {"app": "Slack"}})
                .to_string(), // Mon
            json!({"type": "app_switch", "ts": "2023-10-02T10:00:00Z", "data": {"app": "Chrome"}})
                .to_string(),
        ];

        let patterns = detector.detect_time_patterns(&events);

        // Should find 1 pattern: Slack on Mondays ~9AM
        assert_eq!(patterns.len(), 1);
        let p = &patterns[0];
        assert_eq!(p.pattern_type, PatternType::TimeBasedAction);
        assert!(p.description.contains("Slack"));
        assert!(p.description.contains("Monday"));
        assert!(p.description.contains("9:00"));
        assert_eq!(p.occurrences, 3);
        assert_eq!(p.distinct_days, 3);
        assert_eq!(p.weekday_occurrences, 3);
        assert_eq!(p.work_hour_occurrences, 3);
    }

    #[test]
    fn heavy_usage_pattern_is_not_recommendable_by_default() {
        let detector = PatternDetector::new();
        let pattern = DetectedPattern {
            pattern_id: "p_heavy".to_string(),
            pattern_type: PatternType::AppSequence,
            description: "Heavy usage: Slack".to_string(),
            occurrences: 8,
            distinct_days: 4,
            weekday_occurrences: 0,
            work_hour_occurrences: 0,
            similarity_score: 0.9,
            sample_events: vec![],
            detected_at: Utc::now(),
        };

        assert!(!detector.should_recommend(&pattern));
    }

    #[test]
    fn multi_day_workflow_cycle_is_recommendable() {
        let detector = PatternDetector::new();
        let pattern = DetectedPattern {
            pattern_id: "p_cycle".to_string(),
            pattern_type: PatternType::AppSequence,
            description: "Workflow Cycle: Slack → Notion".to_string(),
            occurrences: 5,
            distinct_days: 3,
            weekday_occurrences: 0,
            work_hour_occurrences: 0,
            similarity_score: 0.95,
            sample_events: vec![],
            detected_at: Utc::now(),
        };

        assert!(detector.should_recommend(&pattern));
    }

    #[test]
    fn app_sequence_records_weekday_and_work_hour_context() {
        let detector = PatternDetector::new();
        let events = vec![
            json!({"type": "app_switch", "ts": "2026-03-02T09:05:00Z", "data": {"app": "Slack"}})
                .to_string(),
            json!({"type": "app_switch", "ts": "2026-03-03T22:15:00Z", "data": {"app": "Slack"}})
                .to_string(),
            json!({"type": "app_switch", "ts": "2026-03-08T10:00:00Z", "data": {"app": "Slack"}})
                .to_string(),
            json!({"type": "app_switch", "ts": "2026-03-04T09:10:00Z", "data": {"app": "Chrome"}})
                .to_string(),
        ];

        let patterns = detector.analyze_with_events(&events);
        let pattern = patterns
            .iter()
            .find(|pattern| pattern.description == "Heavy usage: Slack")
            .expect("slack heavy-usage pattern");

        assert_eq!(pattern.weekday_occurrences, 2);
        assert_eq!(pattern.work_hour_occurrences, 1);
    }
}
