use super::*;
use crate::pattern_detector::{DetectedPattern, PatternType};
use chrono::Utc;
use serial_test::serial;

fn work_pattern() -> DetectedPattern {
    DetectedPattern {
        pattern_id: "p_work".to_string(),
        pattern_type: PatternType::AppSequence,
        description: "Workflow Cycle: Slack -> Notion -> Calendar".to_string(),
        occurrences: 6,
        distinct_days: 3,
        weekday_occurrences: 6,
        work_hour_occurrences: 5,
        similarity_score: 0.92,
        sample_events: vec![
            r#"{"event_type":"app_switch","payload":{"app":"Slack"}}"#.to_string(),
            r#"{"event_type":"app_switch","payload":{"app":"Notion"}}"#.to_string(),
        ],
        detected_at: Utc::now(),
    }
}

fn base_recommendation() -> crate::db::Recommendation {
    crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Meeting Prep Assistant".to_string(),
        summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a meeting prep workflow that uses Slack and Calendar.".to_string(),
        confidence: 0.88,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![
            "Frequency: Found 6 occurrences".to_string(),
            "Span: 4 distinct day(s)".to_string(),
            "policy.reason=strong_work_signals=slack,calendar".to_string(),
            "policy.reason=pattern.context_score=0.83".to_string(),
            "policy.reason=pattern.work_context=strong".to_string(),
        ],
        pattern_id: Some("pattern-work".to_string()),
        last_error: None,
        snoozed_until: None,
        category: CATEGORY_WORK.to_string(),
        business_score: 0.82,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    }
}

fn strong_auto_evidence() -> Vec<String> {
    vec![
        "Frequency: Found 6 occurrences".to_string(),
        "Span: 4 distinct day(s)".to_string(),
        "policy.reason=strong_work_signals=slack,calendar".to_string(),
        "policy.reason=pattern.context_score=0.83".to_string(),
        "policy.reason=pattern.work_context=strong".to_string(),
    ]
}

mod mvp;
mod preferences;
mod queueing;
