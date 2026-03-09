use crate::pattern_detector::DetectedPattern;
use crate::recommendation::AutomationProposal;
use std::collections::HashMap;

#[path = "recommendation_policy/admission.rs"]
mod admission;
#[path = "recommendation_policy/classification.rs"]
mod classification;
#[path = "recommendation_policy/config.rs"]
mod config;
#[path = "recommendation_policy/preferences.rs"]
mod preferences;
#[path = "recommendation_policy/signals.rs"]
mod signals;

use self::config::*;
use self::signals::*;
pub use self::{admission::*, classification::*, preferences::*};

pub const CATEGORY_WORK: &str = "work";
pub const CATEGORY_PERSONAL: &str = "personal";
pub const CATEGORY_SYSTEM: &str = "system";
pub const CATEGORY_UNKNOWN: &str = "unknown";

#[derive(Debug, Clone)]
pub struct RecommendationPolicyDecision {
    pub category: String,
    pub business_score: f64,
    pub accepted: bool,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AutoRecommendationAdmissionDecision {
    pub accepted: bool,
    pub reasons: Vec<String>,
    pub priority_score: f64,
    pub pending_same_category: usize,
    pub pending_limit: usize,
}

#[derive(Debug, Clone, Default)]
pub struct RecommendationPreferenceProfile {
    pub preferred_targets: Vec<String>,
    pub avoided_targets: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RecommendationApprovalReadinessDecision {
    pub ready: bool,
    pub reasons: Vec<String>,
    pub category: String,
    pub business_score: f64,
    pub confidence: f64,
    pub occurrences: Option<u32>,
    pub distinct_days: Option<u32>,
}

pub fn pending_recommendation_display_limit() -> usize {
    std::env::var("ALLVIA_PENDING_RECOMMENDATION_DISPLAY_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .map(|v| v.clamp(1, 100))
        .unwrap_or(5)
}

pub fn auto_recommendation_history_limit() -> i64 {
    std::env::var("ALLVIA_AUTO_RECOMMENDATION_HISTORY_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .map(|v| v.clamp(20, 1000))
        .unwrap_or(200)
}

#[cfg(test)]
#[path = "recommendation_policy/tests/mod.rs"]
mod tests;
