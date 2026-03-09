#![allow(dead_code)]

mod detectors;
mod support;

#[cfg(test)]
mod tests;

use crate::db;
use crate::llm_gateway::LLMClient;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};

pub(crate) use support::{allow_single_app_patterns, env_f64, env_u32};

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
    AppSequence,
    KeywordRepeat,
    FilePattern,
    TimeBasedAction,
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
    pub min_occurrences: u32,
    pub min_similarity: f64,
    pub lookback_days: i64,
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

    pub fn analyze(&self) -> Vec<DetectedPattern> {
        let hours = self.config.lookback_days * 24;
        let events = match db::get_recent_events(hours) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        self.analyze_with_events(&events)
    }

    pub fn analyze_with_events(&self, events: &[String]) -> Vec<DetectedPattern> {
        if events.is_empty() {
            return Vec::new();
        }

        let mut patterns = Vec::new();
        patterns.extend(self.detect_app_sequences(events));
        patterns.extend(self.detect_keyword_patterns(events));
        patterns.extend(self.detect_file_patterns(events));
        patterns.extend(self.detect_time_patterns(events));

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

    pub async fn analyze_async(&self) -> Vec<DetectedPattern> {
        let hours = self.config.lookback_days * 24;
        let events = match db::get_recent_events(hours) {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };

        let patterns = self.analyze_with_events(&events);
        if self.llm_client.is_some() {
            return self.merge_similar_patterns(patterns).await;
        }

        patterns
    }

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
                            current_group.occurrences += patterns[j].occurrences;
                            current_group.distinct_days =
                                current_group.distinct_days.max(patterns[j].distinct_days);
                            current_group
                                .sample_events
                                .extend(patterns[j].sample_events.clone());
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
