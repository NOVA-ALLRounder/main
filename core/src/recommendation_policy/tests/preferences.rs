use super::*;

#[test]
fn refinement_feedback_builds_destination_preference_profile() {
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Slack Digest".to_string(),
        summary: "digest".to_string(),
        trigger: "digest".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Send digest to Telegram.".to_string(),
        confidence: 0.8,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_feedback".to_string()),
        last_error: None,
        snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.8,
        feedback_status: Some("refine".to_string()),
        feedback_note: Some("텔레그램 말고 노션으로 저장해줘".to_string()),
        feedback_count: 1,
        last_feedback_at: Some(chrono::Utc::now().to_rfc3339()),
    }];

    let profile = build_recommendation_preference_profile(&existing);
    assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
    assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
}

#[test]
fn approved_and_rejected_reviews_contribute_soft_preferences() {
    let existing = vec![
        crate::db::Recommendation {
            id: 1,
            status: "approved".to_string(),
            title: "Meeting Notes".to_string(),
            summary: "Save notes to Notion".to_string(),
            trigger: "meeting".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
            confidence: 0.8,
            workflow_id: Some("wf-1".to_string()),
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_notion".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
        crate::db::Recommendation {
            id: 2,
            status: "rejected".to_string(),
            title: "Telegram Digest".to_string(),
            summary: "Send digest to Telegram".to_string(),
            trigger: "digest".to_string(),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some("p_telegram".to_string()),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.8,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        },
    ];

    let profile = build_recommendation_preference_profile(&existing);
    assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
    assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
}

#[test]
fn apply_recommendation_preferences_injects_preference_hints() {
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Slack Digest".to_string(),
        summary: "digest".to_string(),
        trigger: "digest".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Send digest to Telegram.".to_string(),
        confidence: 0.8,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_feedback".to_string()),
        last_error: None,
        snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.8,
        feedback_status: Some("refine".to_string()),
        feedback_note: Some("텔레그램 말고 노션으로 저장해줘".to_string()),
        feedback_count: 1,
        last_feedback_at: Some(chrono::Utc::now().to_rfc3339()),
    }];
    let mut proposal = AutomationProposal {
        title: "Meeting Prep Assistant".to_string(),
        summary: "회의 준비".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.9,
        n8n_prompt: "Create a workflow that sends meeting notes to Telegram.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work_4".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };

    let profile = apply_recommendation_preferences(&mut proposal, &existing);

    assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
    assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
    assert!(proposal.n8n_prompt.contains("Prefer Notion"));
    assert!(proposal.n8n_prompt.contains("Avoid Telegram"));
    assert!(proposal
        .evidence
        .iter()
        .any(|entry| entry.contains("preference.prefer=notion")));
    assert!(proposal
        .evidence
        .iter()
        .any(|entry| entry.contains("preference.conflict=telegram")));
}

#[test]
fn apply_recommendation_preferences_penalizes_avoided_targets() {
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "rejected".to_string(),
        title: "Telegram Digest".to_string(),
        summary: "Send digest to Telegram".to_string(),
        trigger: "digest".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
        confidence: 0.8,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_feedback_reject".to_string()),
        last_error: None,
        snoozed_until: None,
        category: CATEGORY_WORK.to_string(),
        business_score: 0.8,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    }];
    let mut proposal = AutomationProposal {
        title: "Status Digest".to_string(),
        summary: "업무 요약".to_string(),
        trigger: "digest".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.90,
        n8n_prompt: "Create a workflow that sends a work digest to Telegram.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work_penalty".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };

    let profile = apply_recommendation_preferences(&mut proposal, &existing);

    assert_eq!(profile.avoided_targets, vec![TARGET_TELEGRAM.to_string()]);
    assert!((proposal.business_score - 0.78).abs() < 1e-9);
    assert!((proposal.confidence - 0.86).abs() < 1e-9);
    assert!(proposal
        .evidence
        .iter()
        .any(|entry| entry == "preference.business_score_adjusted=0.78"));
}

#[test]
fn apply_recommendation_preferences_rewards_preferred_targets() {
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "approved".to_string(),
        title: "Notion Notes".to_string(),
        summary: "Save notes to Notion".to_string(),
        trigger: "meeting".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a workflow that writes meeting notes to Notion.".to_string(),
        confidence: 0.8,
        workflow_id: Some("wf-1".to_string()),
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_feedback_approve".to_string()),
        last_error: None,
        snoozed_until: None,
        category: CATEGORY_WORK.to_string(),
        business_score: 0.8,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    }];
    let mut proposal = AutomationProposal {
        title: "Meeting Prep".to_string(),
        summary: "회의 준비".to_string(),
        trigger: "meeting".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.90,
        n8n_prompt: "Create a workflow that stores meeting prep notes in Notion.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work_bonus".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.82,
    };

    let profile = apply_recommendation_preferences(&mut proposal, &existing);

    assert_eq!(profile.preferred_targets, vec![TARGET_NOTION.to_string()]);
    assert!((proposal.business_score - 0.86).abs() < 1e-9);
    assert!((proposal.confidence - 0.91).abs() < 1e-9);
    assert!(proposal
        .evidence
        .iter()
        .any(|entry| entry == "preference.match=notion"));
}

#[test]
fn snoozed_recommendations_do_not_consume_pending_limit() {
    let proposal = AutomationProposal {
        title: "Meeting Prep Assistant".to_string(),
        summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.9,
        n8n_prompt: "Create a meeting prep workflow.".to_string(),
        evidence: strong_auto_evidence(),
        pattern_id: Some("p_work_2".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };
    let snoozed_until = (chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339();
    let existing = (0..5)
        .map(|idx| crate::db::Recommendation {
            id: idx + 1,
            status: "pending".to_string(),
            title: format!("Later Workflow {}", idx),
            summary: "Existing".to_string(),
            trigger: format!("later {}", idx),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a later workflow.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some(format!("p_later_{}", idx)),
            last_error: None,
            snoozed_until: Some(snoozed_until.clone()),
            category: CATEGORY_WORK.to_string(),
            business_score: 0.78,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        })
        .collect::<Vec<_>>();

    let decision = admit_auto_recommendation(&proposal, &existing);

    assert!(decision.accepted);
    assert_eq!(decision.pending_same_category, 0);
}

#[test]
fn snoozed_duplicate_recommendation_is_still_suppressed() {
    let proposal = AutomationProposal {
        title: "Meeting Prep Assistant".to_string(),
        summary: "회의 준비".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.9,
        n8n_prompt: "Create a meeting prep workflow.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work_2".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Meeting Prep Assistant".to_string(),
        summary: "Existing".to_string(),
        trigger: "meeting prep".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a meeting prep workflow.".to_string(),
        confidence: 0.8,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_work_2".to_string()),
        last_error: None,
        snoozed_until: Some((chrono::Utc::now() + chrono::Duration::hours(24)).to_rfc3339()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.78,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    }];

    let decision = admit_auto_recommendation(&proposal, &existing);

    assert!(!decision.accepted);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("snoozed_pattern_id")));
}
