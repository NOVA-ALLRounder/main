use super::*;

#[test]
fn work_pattern_is_accepted_for_mvp() {
    let mut proposal = AutomationProposal {
        title: "회의 준비 자동화".to_string(),
        summary: "Slack과 Calendar 기반 회의 준비".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.85,
        n8n_prompt: "Create a meeting prep workflow from Slack and Calendar.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work".to_string()),
        category: CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };

    let decision = apply_mvp_policy(&mut proposal, Some(&work_pattern()));
    assert!(decision.accepted);
    assert_eq!(proposal.category, CATEGORY_WORK);
    assert!(proposal.business_score >= 0.55);
}

#[test]
fn personal_pattern_is_rejected_for_mvp() {
    let mut proposal = AutomationProposal {
        title: "유튜브 정리".to_string(),
        summary: "YouTube와 Spotify 사용을 자동 정리".to_string(),
        trigger: "personal media".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.8,
        n8n_prompt: "Create a workflow for YouTube and Spotify usage.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_personal".to_string()),
        category: CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };

    let decision = apply_mvp_policy(&mut proposal, None);
    assert!(!decision.accepted);
    assert_eq!(proposal.category, CATEGORY_PERSONAL);
}

#[test]
fn support_only_pattern_requires_work_context_for_mvp() {
    let mut proposal = AutomationProposal {
        title: "코딩 포커스 자동화".to_string(),
        summary: "VSCode와 Terminal 사용을 감지해 집중 모드를 켭니다.".to_string(),
        trigger: "coding focus".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.82,
        n8n_prompt: "Create a workflow that reacts to VSCode and Terminal usage.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_support_only".to_string()),
        category: CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };
    let off_hours_pattern = DetectedPattern {
        pattern_id: "p_support_only".to_string(),
        pattern_type: PatternType::AppSequence,
        description: "Workflow Cycle: VSCode -> Terminal".to_string(),
        occurrences: 6,
        distinct_days: 4,
        weekday_occurrences: 2,
        work_hour_occurrences: 0,
        similarity_score: 0.91,
        sample_events: vec![],
        detected_at: Utc::now(),
    };

    let decision = apply_mvp_policy(&mut proposal, Some(&off_hours_pattern));

    assert!(!decision.accepted);
    assert_eq!(proposal.category, CATEGORY_UNKNOWN);
    assert!(proposal.business_score < 0.55);
}

#[test]
fn strong_work_context_can_promote_support_only_pattern() {
    let mut proposal = AutomationProposal {
        title: "코딩 포커스 자동화".to_string(),
        summary: "VSCode와 Terminal 사용을 감지해 집중 모드를 켭니다.".to_string(),
        trigger: "coding focus".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.82,
        n8n_prompt: "Create a workflow that reacts to VSCode and Terminal usage.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_support_work".to_string()),
        category: CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };
    let work_hours_pattern = DetectedPattern {
        pattern_id: "p_support_work".to_string(),
        pattern_type: PatternType::AppSequence,
        description: "Workflow Cycle: VSCode -> Terminal".to_string(),
        occurrences: 6,
        distinct_days: 4,
        weekday_occurrences: 6,
        work_hour_occurrences: 5,
        similarity_score: 0.91,
        sample_events: vec![],
        detected_at: Utc::now(),
    };

    let decision = apply_mvp_policy(&mut proposal, Some(&work_hours_pattern));

    assert!(decision.accepted);
    assert_eq!(proposal.category, CATEGORY_WORK);
    assert!(proposal.business_score >= 0.55);
    assert!(proposal
        .evidence
        .iter()
        .any(|entry| entry.contains("pattern.context_score")));
}

#[test]
#[serial]
fn list_category_filter_defaults_to_work_when_browsing_disabled() {
    std::env::remove_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING");
    std::env::set_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY", "all");

    assert_eq!(
        normalize_list_category_filter(Some("all")).as_deref(),
        Some(CATEGORY_WORK)
    );
    assert_eq!(
        normalize_list_category_filter(Some("personal")).as_deref(),
        Some(CATEGORY_WORK)
    );
    assert_eq!(
        normalize_list_category_filter(None).as_deref(),
        Some(CATEGORY_WORK)
    );

    std::env::remove_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY");
}

#[test]
#[serial]
fn list_category_filter_allows_requested_category_when_browsing_enabled() {
    std::env::set_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING", "true");
    std::env::set_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY", "work");

    assert_eq!(
        normalize_list_category_filter(Some("personal")).as_deref(),
        Some(CATEGORY_PERSONAL)
    );
    assert_eq!(
        normalize_list_category_filter(Some("all")).as_deref(),
        Some("all")
    );
    assert_eq!(
        normalize_list_category_filter(None).as_deref(),
        Some(CATEGORY_WORK)
    );

    std::env::remove_var("ALLVIA_ENABLE_RECOMMENDATION_CATEGORY_BROWSING");
    std::env::remove_var("ALLVIA_RECOMMENDATION_LIST_DEFAULT_CATEGORY");
}
