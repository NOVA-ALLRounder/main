// Pattern Detector - Detect repetitive user behavior patterns

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

/// Pattern Detector - Analyzes user behavior to find automation opportunities
pub struct PatternDetector {
    patterns: Arc<RwLock<Vec<DetectedPattern>>>,
    action_history: Arc<RwLock<Vec<UserAction>>>,
    db_path: Option<String>,
    config: PatternConfig,
}

#[derive(Debug, Clone)]
struct PatternConfig {
    min_repetitions: usize,     // Minimum repetitions to consider a pattern
    _time_window_secs: u64,      // Time window to look for patterns
    _similarity_threshold: f32,  // Similarity threshold (0.0-1.0)
}

impl Default for PatternConfig {
    fn default() -> Self {
        Self {
            min_repetitions: 3,
            _time_window_secs: 3600, // 1 hour
            _similarity_threshold: 0.8,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAction {
    pub timestamp: u64,
    pub action_type: String,
    pub target: String,
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub actions: Vec<UserAction>,
    pub frequency: usize,
    pub last_occurrence: u64,
    pub confidence: f32,
    pub automation_suggestion: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternType {
    Sequential,    // Actions in sequence
    Repetitive,    // Same action repeated
    Periodic,      // Actions at regular intervals
    Conditional,   // If-then patterns
}

impl PatternDetector {
    pub async fn new() -> Result<Self> {
        Ok(Self {
            patterns: Arc::new(RwLock::new(Vec::new())),
            action_history: Arc::new(RwLock::new(Vec::new())),
            db_path: None,
            config: PatternConfig::default(),
        })
    }

    /// Initialize with database connection
    pub async fn with_db(db_path: &str) -> Result<Self> {
        let mut detector = Self::new().await?;
        detector.db_path = Some(db_path.to_string());
        Ok(detector)
    }

    /// Record a user action
    pub async fn record_action(&self, action: UserAction) -> Result<()> {
        log::debug!("Recording action: {:?}", action);

        // Add to history
        {
            let mut history = self.action_history.write().await;
            history.push(action.clone());

            // Keep only recent history (last 1000 actions)
            if history.len() > 1000 {
                history.drain(0..100);
            }
        }

        // Save to database if enabled
        if let Some(db_path) = &self.db_path {
            self.save_action_to_db(db_path, &action).await?;
        }

        // Trigger pattern detection
        self.detect_patterns().await?;

        Ok(())
    }

    /// Detect patterns in action history
    pub async fn detect_patterns(&self) -> Result<Vec<DetectedPattern>> {
        let history = self.action_history.read().await;

        if history.len() < self.config.min_repetitions {
            return Ok(vec![]);
        }

        let mut detected = Vec::new();

        // Detect sequential patterns
        if let Some(sequential) = self.detect_sequential_pattern(&history) {
            detected.push(sequential);
        }

        // Detect repetitive patterns
        if let Some(repetitive) = self.detect_repetitive_pattern(&history) {
            detected.push(repetitive);
        }

        // Detect periodic patterns
        if let Some(periodic) = self.detect_periodic_pattern(&history) {
            detected.push(periodic);
        }

        // Update stored patterns
        {
            let mut patterns = self.patterns.write().await;
            for pattern in &detected {
                // Check if pattern already exists
                if let Some(existing) = patterns.iter_mut().find(|p| p.id == pattern.id) {
                    // Update existing pattern
                    existing.frequency += 1;
                    existing.last_occurrence = pattern.last_occurrence;
                    existing.confidence = pattern.confidence;
                } else {
                    // Add new pattern
                    patterns.push(pattern.clone());
                }
            }

            // Remove old patterns (not seen in 7 days)
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            patterns.retain(|p| now - p.last_occurrence < 7 * 24 * 3600);
        }

        // Save patterns to database
        if let Some(db_path) = &self.db_path {
            for pattern in &detected {
                self.save_pattern_to_db(db_path, pattern).await?;
            }
        }

        Ok(detected)
    }

    /// Detect sequential pattern (A ??B ??C)
    fn detect_sequential_pattern(&self, history: &[UserAction]) -> Option<DetectedPattern> {
        if history.len() < 3 {
            return None;
        }

        // Look for sequences of 3+ actions
        let window_size = 3;
        let mut sequence_counts: HashMap<String, usize> = HashMap::new();

        for window in history.windows(window_size) {
            let sequence_key = window
                .iter()
                .map(|a| format!("{}:{}", a.action_type, a.target))
                .collect::<Vec<_>>()
                .join(" -> ");

            *sequence_counts.entry(sequence_key).or_insert(0) += 1;
        }

        // Find most common sequence
        if let Some((sequence, count)) = sequence_counts
            .iter()
            .filter(|(_, &c)| c >= self.config.min_repetitions)
            .max_by_key(|(_, &c)| c)
        {
            let actions: Vec<UserAction> = history[history.len() - window_size..]
                .iter()
                .cloned()
                .collect();

            Some(DetectedPattern {
                id: format!("seq_{}", uuid::Uuid::new_v4()),
                pattern_type: PatternType::Sequential,
                actions,
                frequency: *count,
                last_occurrence: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                confidence: (*count as f32 / history.len() as f32).min(1.0),
                automation_suggestion: format!(
                    "Automate sequence: {}. Detected {} times.",
                    sequence, count
                ),
            })
        } else {
            None
        }
    }

    /// Detect repetitive pattern (A, A, A, ...)
    fn detect_repetitive_pattern(&self, history: &[UserAction]) -> Option<DetectedPattern> {
        let mut action_counts: HashMap<String, Vec<&UserAction>> = HashMap::new();

        for action in history {
            let key = format!("{}:{}", action.action_type, action.target);
            action_counts.entry(key).or_default().push(action);
        }

        // Find most repeated action
        if let Some((key, actions)) = action_counts
            .iter()
            .filter(|(_, v)| v.len() >= self.config.min_repetitions)
            .max_by_key(|(_, v)| v.len())
        {
            Some(DetectedPattern {
                id: format!("rep_{}", uuid::Uuid::new_v4()),
                pattern_type: PatternType::Repetitive,
                actions: actions.iter().map(|&a| a.clone()).collect(),
                frequency: actions.len(),
                last_occurrence: actions.last().unwrap().timestamp,
                confidence: (actions.len() as f32 / history.len() as f32).min(1.0),
                automation_suggestion: format!(
                    "Action '{}' repeated {} times. Consider batch automation.",
                    key,
                    actions.len()
                ),
            })
        } else {
            None
        }
    }

    /// Detect periodic pattern (every N minutes/hours)
    fn detect_periodic_pattern(&self, history: &[UserAction]) -> Option<DetectedPattern> {
        if history.len() < self.config.min_repetitions {
            return None;
        }

        // Group actions by type
        let mut action_groups: HashMap<String, Vec<&UserAction>> = HashMap::new();
        for action in history {
            let key = format!("{}:{}", action.action_type, action.target);
            action_groups.entry(key).or_default().push(action);
        }

        // Look for periodic patterns
        for (key, actions) in action_groups {
            if actions.len() < self.config.min_repetitions {
                continue;
            }

            // Calculate time intervals between actions
            let mut intervals = Vec::new();
            for window in actions.windows(2) {
                let interval = window[1].timestamp - window[0].timestamp;
                intervals.push(interval);
            }

            if intervals.is_empty() {
                continue;
            }

            // Check if intervals are similar (within 20% variance)
            let avg_interval = intervals.iter().sum::<u64>() / intervals.len() as u64;
            let variance = intervals
                .iter()
                .map(|&i| {
                    let diff = if i > avg_interval {
                        i - avg_interval
                    } else {
                        avg_interval - i
                    };
                    diff as f32 / avg_interval as f32
                })
                .sum::<f32>()
                / intervals.len() as f32;

            if variance < 0.2 {
                // Periodic pattern detected
                return Some(DetectedPattern {
                    id: format!("per_{}", uuid::Uuid::new_v4()),
                    pattern_type: PatternType::Periodic,
                    actions: actions.iter().map(|&a| a.clone()).collect(),
                    frequency: actions.len(),
                    last_occurrence: actions.last().unwrap().timestamp,
                    confidence: 1.0 - variance,
                    automation_suggestion: format!(
                        "Action '{}' occurs every {} seconds. Consider scheduling.",
                        key,
                        avg_interval
                    ),
                });
            }
        }

        None
    }

    /// Get all detected patterns
    pub async fn get_patterns(&self) -> Vec<DetectedPattern> {
        self.patterns.read().await.clone()
    }

    /// Get patterns by type
    pub async fn get_patterns_by_type(&self, pattern_type: PatternType) -> Vec<DetectedPattern> {
        self.patterns
            .read()
            .await
            .iter()
            .filter(|p| p.pattern_type == pattern_type)
            .cloned()
            .collect()
    }

    /// Get automation suggestions
    pub async fn get_automation_suggestions(&self) -> Vec<String> {
        self.patterns
            .read()
            .await
            .iter()
            .map(|p| p.automation_suggestion.clone())
            .collect()
    }

    /// Save action to database
    async fn save_action_to_db(&self, db_path: &str, action: &UserAction) -> Result<()> {
        let db_path = db_path.to_string();
        let action_json = serde_json::to_string(action)?;
        let timestamp = action.timestamp;
        let action_type = action.action_type.clone();

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            conn.execute(
                "INSERT INTO activity_logs (timestamp, app_name, window_title, duration_ms) VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    timestamp,
                    &action_type,
                    &action_json,
                    0
                ],
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    /// Save pattern to database
    async fn save_pattern_to_db(&self, db_path: &str, pattern: &DetectedPattern) -> Result<()> {
        let db_path = db_path.to_string();
        let pattern_id = pattern.id.clone();
        let pattern_type = format!("{:?}", pattern.pattern_type);
        let frequency = pattern.frequency;
        let confidence = pattern.confidence;
        let suggestion = pattern.automation_suggestion.clone();
        let detected_at = pattern.last_occurrence;

        tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&db_path)?;

            conn.execute(
                "INSERT OR REPLACE INTO patterns (pattern_id, pattern_type, frequency, confidence, suggestion, detected_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                rusqlite::params![
                    &pattern_id,
                    pattern_type,
                    frequency,
                    confidence,
                    &suggestion,
                    detected_at
                ],
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .await??;

        Ok(())
    }

    /// Clear all patterns
    pub async fn clear_patterns(&self) {
        self.patterns.write().await.clear();
        self.action_history.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pattern_detector_creation() {
        let detector = PatternDetector::new().await.unwrap();
        let patterns = detector.get_patterns().await;
        assert_eq!(patterns.len(), 0);
    }

    #[tokio::test]
    async fn test_record_action() {
        let detector = PatternDetector::new().await.unwrap();

        let action = UserAction {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            action_type: "click".to_string(),
            target: "button".to_string(),
            parameters: HashMap::new(),
        };

        detector.record_action(action).await.unwrap();

        let history = detector.action_history.read().await;
        assert_eq!(history.len(), 1);
    }

    #[tokio::test]
    async fn test_detect_repetitive_pattern() {
        let detector = PatternDetector::new().await.unwrap();

        // Record same action 5 times
        for _ in 0..5 {
            let action = UserAction {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                action_type: "click".to_string(),
                target: "submit_button".to_string(),
                parameters: HashMap::new(),
            };
            detector.record_action(action).await.unwrap();
        }

        let patterns = detector.get_patterns().await;
        assert!(!patterns.is_empty());

        let repetitive = detector
            .get_patterns_by_type(PatternType::Repetitive)
            .await;
        assert!(!repetitive.is_empty());
    }

    #[tokio::test]
    async fn test_automation_suggestions() {
        let detector = PatternDetector::new().await.unwrap();

        // Record repetitive actions
        for i in 0..4 {
            let action = UserAction {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + i,
                action_type: "type_text".to_string(),
                target: "input_field".to_string(),
                parameters: HashMap::new(),
            };
            detector.record_action(action).await.unwrap();
        }

        let suggestions = detector.get_automation_suggestions().await;
        assert!(!suggestions.is_empty());
    }

    #[tokio::test]
    async fn test_clear_patterns() {
        let detector = PatternDetector::new().await.unwrap();

        // Add some actions
        for _ in 0..3 {
            let action = UserAction {
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs(),
                action_type: "test".to_string(),
                target: "test".to_string(),
                parameters: HashMap::new(),
            };
            detector.record_action(action).await.unwrap();
        }

        detector.clear_patterns().await;

        let patterns = detector.get_patterns().await;
        let history = detector.action_history.read().await;

        assert_eq!(patterns.len(), 0);
        assert_eq!(history.len(), 0);
    }
}
