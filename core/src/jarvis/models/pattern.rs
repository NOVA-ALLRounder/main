// Pattern detection models

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedPattern {
    pub id: String,
    pub pattern_type: PatternType,
    pub occurrences: Vec<Occurrence>,
    pub confidence: f32,
    pub suggested_automation: Option<String>,
    pub status: PatternStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PatternType {
    AppLaunch { app: String },
    FileOpen { path: PathBuf },
    WebsiteVisit { url: String },
    ClickSequence { actions: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Occurrence {
    pub timestamp: i64,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PatternStatus {
    Detected,
    Suggested,
    Automated,
    Dismissed,
}

impl DetectedPattern {
    pub fn new(pattern_type: PatternType) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            pattern_type,
            occurrences: Vec::new(),
            confidence: 0.0,
            suggested_automation: None,
            status: PatternStatus::Detected,
        }
    }

    pub fn add_occurrence(&mut self, occurrence: Occurrence) {
        self.occurrences.push(occurrence);
        self.recalculate_confidence();
    }

    fn recalculate_confidence(&mut self) {
        // Simple confidence calculation based on occurrence count
        let count = self.occurrences.len() as f32;
        self.confidence = (count / (count + 2.0)).min(1.0);
    }

    pub fn should_suggest(&self) -> bool {
        self.occurrences.len() >= 3 && self.confidence >= 0.7
    }
}
