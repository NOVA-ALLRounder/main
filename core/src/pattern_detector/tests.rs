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
    let events = vec![
        json!({"type": "app_switch", "ts": "2023-10-02T09:00:00Z", "data": {"app": "Slack"}})
            .to_string(),
        json!({"type": "app_switch", "ts": "2023-10-09T09:15:00Z", "data": {"app": "Slack"}})
            .to_string(),
        json!({"type": "app_switch", "ts": "2023-10-16T09:05:00Z", "data": {"app": "Slack"}})
            .to_string(),
        json!({"type": "app_switch", "ts": "2023-10-02T10:00:00Z", "data": {"app": "Chrome"}})
            .to_string(),
    ];

    let patterns = detector.detect_time_patterns(&events);

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
