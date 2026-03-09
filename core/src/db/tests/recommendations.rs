use super::*;

#[test]
#[serial]
fn test_recommendation_review_status_transitions() {
    init().ok();
    clear_recommendations_for_tests();

    let unique = format!("status-transition-test-{}", uuid::Uuid::new_v4());
    let proposal = AutomationProposal {
        title: unique.clone(),
        summary: "status transition test".to_string(),
        trigger: "unit-test".to_string(),
        actions: vec!["noop".to_string()],
        confidence: 0.1,
        n8n_prompt: "noop".to_string(),
        evidence: vec![],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };
    let _ = insert_recommendation(&proposal);

    let rows = get_recommendations_with_filter(Some("all")).unwrap_or_default();
    let Some(rec) = rows.into_iter().find(|r| r.title == unique) else {
        eprintln!("skip: could not resolve inserted recommendation for transition test");
        return;
    };

    assert!(update_recommendation_review_status(rec.id, "approved").is_ok());
    assert!(update_recommendation_review_status(rec.id, "pending").is_err());
    assert!(update_recommendation_review_status(rec.id, "rejected").is_ok());
    assert!(update_recommendation_review_status(rec.id, "later").is_err());
}

#[test]
#[serial]
fn test_recommendation_snooze_roundtrip_and_metrics() {
    init().ok();
    clear_recommendations_for_tests();

    let unique = format!("snooze-transition-test-{}", uuid::Uuid::new_v4());
    let proposal = AutomationProposal {
        title: unique.clone(),
        summary: "snooze transition test".to_string(),
        trigger: "unit-test".to_string(),
        actions: vec!["noop".to_string()],
        confidence: 0.1,
        n8n_prompt: "noop".to_string(),
        evidence: vec![],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.7,
    };
    let _ = insert_recommendation(&proposal);

    let rows = get_recommendations_with_filter(Some("all")).unwrap_or_default();
    let rec = rows
        .into_iter()
        .find(|r| r.title == unique)
        .expect("recommendation");

    snooze_recommendation(rec.id, 24).expect("snooze recommendation");
    let snoozed = get_recommendation(rec.id)
        .expect("get recommendation")
        .expect("row");
    assert!(snoozed.snoozed_until.is_some());

    let metrics = get_recommendation_metrics().expect("metrics");
    assert_eq!(metrics.pending, 0);
    assert_eq!(metrics.later, 1);

    restore_recommendation(rec.id).expect("restore recommendation");
    let restored = get_recommendation(rec.id)
        .expect("get recommendation")
        .expect("row");
    assert!(restored.snoozed_until.is_none());

    let metrics = get_recommendation_metrics().expect("metrics");
    assert_eq!(metrics.pending, 1);
    assert_eq!(metrics.later, 0);
}

#[test]
#[serial]
fn test_snooze_recommendation_rejects_non_pending_status() {
    init().ok();
    clear_recommendations_for_tests();

    let unique = format!("snooze-approved-test-{}", uuid::Uuid::new_v4());
    let proposal = AutomationProposal {
        title: unique.clone(),
        summary: "snooze approved recommendation test".to_string(),
        trigger: "unit-test".to_string(),
        actions: vec!["noop".to_string()],
        confidence: 0.1,
        n8n_prompt: "noop".to_string(),
        evidence: vec![],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.7,
    };
    let _ = insert_recommendation(&proposal);

    let rec = get_recommendations_with_filter(Some("all"))
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.title == unique)
        .expect("recommendation");
    update_recommendation_review_status(rec.id, "approved").expect("approve recommendation");

    let err =
        snooze_recommendation(rec.id, 24).expect_err("approved recommendation should not snooze");
    assert!(err.to_string().contains("cannot snooze recommendation"));

    let err =
        restore_recommendation(rec.id).expect_err("approved recommendation should not restore");
    assert!(err.to_string().contains("cannot restore recommendation"));
}

#[test]
#[serial]
fn test_recommendation_feedback_roundtrip() {
    init().ok();
    clear_recommendations_for_tests();

    let unique = format!("feedback-test-{}", uuid::Uuid::new_v4());
    let proposal = AutomationProposal {
        title: unique.clone(),
        summary: "recommendation feedback test".to_string(),
        trigger: "unit-test".to_string(),
        actions: vec!["noop".to_string()],
        confidence: 0.1,
        n8n_prompt: "noop".to_string(),
        evidence: vec![],
        pattern_id: Some("feedback-pattern".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.7,
    };
    let _ = insert_recommendation(&proposal);

    let rec = get_recommendations_with_filter(Some("all"))
        .unwrap_or_default()
        .into_iter()
        .find(|r| r.title == unique)
        .expect("recommendation");

    assert!(
        record_recommendation_feedback(rec.id, "refine", "텔레그램 말고 노션으로 보내줘")
            .expect("record feedback")
    );

    let updated = get_recommendation(rec.id)
        .expect("get recommendation")
        .expect("row");
    assert_eq!(updated.feedback_status.as_deref(), Some("refine"));
    assert_eq!(
        updated.feedback_note.as_deref(),
        Some("텔레그램 말고 노션으로 보내줘")
    );
    assert_eq!(updated.feedback_count, 1);
    assert!(updated.last_feedback_at.is_some());
}
