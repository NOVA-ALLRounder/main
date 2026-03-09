use super::*;

#[test]
fn manual_recommendation_is_ready_without_auto_gate() {
    let mut rec = base_recommendation();
    rec.pattern_id = None;
    rec.category = CATEGORY_UNKNOWN.to_string();
    rec.business_score = 0.0;

    let decision = evaluate_recommendation_approval_readiness(&rec);

    assert!(decision.ready);
    assert!(decision.reasons.is_empty());
}

#[test]
fn auto_recommendation_requires_strong_evidence_for_approval() {
    let mut rec = base_recommendation();
    rec.confidence = 0.74;
    rec.business_score = 0.61;
    rec.summary = "Detected 3 repeats across 1 distinct day(s).".to_string();
    rec.evidence = vec![
        "Frequency: Found 3 occurrences".to_string(),
        "Span: 1 distinct day(s)".to_string(),
        "policy.reason=support_work_signals=terminal,vscode".to_string(),
    ];

    let decision = evaluate_recommendation_approval_readiness(&rec);

    assert!(!decision.ready);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("Business score")));
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("Confidence")));
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("distinct days")));
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("strong work evidence")));
}

#[test]
fn auto_recommendation_with_work_context_is_ready_for_approval() {
    let rec = base_recommendation();

    let decision = evaluate_recommendation_approval_readiness(&rec);

    assert!(decision.ready);
    assert_eq!(decision.occurrences, Some(6));
    assert_eq!(decision.distinct_days, Some(4));
}

#[test]
fn auto_proposal_requires_strong_evidence_for_queueing() {
    let proposal = AutomationProposal {
        title: "Weak Queue Candidate".to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: "weak".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.74,
        n8n_prompt: "Create a workflow.".to_string(),
        evidence: vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=vscode,terminal".to_string(),
        ],
        pattern_id: Some("weak-queue".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.61,
    };

    let decision = evaluate_auto_recommendation_readiness(&proposal);

    assert!(!decision.ready);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("queue threshold")));
}

#[test]
fn auto_proposal_with_strong_evidence_is_ready_for_queueing() {
    let proposal = AutomationProposal {
        title: "Ready Queue Candidate".to_string(),
        summary: "Detected 6 repeats across 4 distinct day(s).".to_string(),
        trigger: "ready".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.88,
        n8n_prompt: "Create a workflow.".to_string(),
        evidence: strong_auto_evidence(),
        pattern_id: Some("ready-queue".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.82,
    };

    let decision = evaluate_auto_recommendation_readiness(&proposal);

    assert!(decision.ready);
    assert_eq!(decision.occurrences, Some(6));
    assert_eq!(decision.distinct_days, Some(4));
}

#[test]
fn auto_recommendation_queue_rejects_duplicate_pending_pattern() {
    let proposal = AutomationProposal {
        title: "Work Start Checklist".to_string(),
        summary: "Slack과 Notion 체크리스트".to_string(),
        trigger: "Workflow Cycle".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.88,
        n8n_prompt: "Create a work start workflow.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.82,
    };
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Work Start Checklist".to_string(),
        summary: "Existing".to_string(),
        trigger: "Workflow Cycle".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a work start workflow.".to_string(),
        confidence: 0.84,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_work".to_string()),
        last_error: None,
        snoozed_until: None,
        category: CATEGORY_WORK.to_string(),
        business_score: 0.8,
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
        .any(|reason| reason.contains("duplicate_pattern_id")));
}

#[test]
fn auto_recommendation_queue_rejects_when_pending_limit_reached() {
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
    let existing = (0..5)
        .map(|idx| crate::db::Recommendation {
            id: idx + 1,
            status: "pending".to_string(),
            title: format!("Daily Briefing {}", idx),
            summary: "Existing".to_string(),
            trigger: format!("daily briefing {}", idx),
            actions: vec!["n8n Workflow".to_string()],
            n8n_prompt: "Create a daily briefing workflow.".to_string(),
            confidence: 0.8,
            workflow_id: None,
            workflow_json: None,
            evidence: vec![],
            pattern_id: Some(format!("p_work_{}", idx)),
            last_error: None,
            snoozed_until: None,
            category: CATEGORY_WORK.to_string(),
            business_score: 0.78,
            feedback_status: None,
            feedback_note: None,
            feedback_count: 0,
            last_feedback_at: None,
        })
        .collect::<Vec<_>>();

    let decision = admit_auto_recommendation(&proposal, &existing);

    assert!(!decision.accepted);
    assert_eq!(decision.pending_same_category, 5);
    assert_eq!(decision.pending_limit, 5);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("pending_limit_reached")));
}

#[test]
fn non_work_auto_recommendation_queue_is_disabled_by_default() {
    let proposal = AutomationProposal {
        title: "Downloads Cleanup".to_string(),
        summary: "downloads folder cleanup".to_string(),
        trigger: "downloads cleanup".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.83,
        n8n_prompt: "Create a downloads cleanup workflow.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_system".to_string()),
        category: CATEGORY_SYSTEM.to_string(),
        business_score: 0.32,
    };

    let decision = admit_auto_recommendation(&proposal, &[]);

    assert!(!decision.accepted);
    assert_eq!(decision.pending_limit, 0);
    assert!(decision
        .reasons
        .iter()
        .any(|reason| reason.contains("pending_limit_reached")));
}

#[test]
fn rejected_history_blocks_similar_auto_recommendation() {
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
        status: "rejected".to_string(),
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
        snoozed_until: None,
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
        .any(|reason| reason.contains("history.rejected_pattern_id")));
}

#[test]
fn approved_history_blocks_duplicate_auto_recommendation() {
    let proposal = AutomationProposal {
        title: "Daily Briefing".to_string(),
        summary: "업무 브리핑".to_string(),
        trigger: "daily briefing".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.9,
        n8n_prompt: "Create a daily briefing workflow.".to_string(),
        evidence: vec![],
        pattern_id: Some("p_work_3".to_string()),
        category: CATEGORY_WORK.to_string(),
        business_score: 0.86,
    };
    let existing = vec![crate::db::Recommendation {
        id: 1,
        status: "approved".to_string(),
        title: "Daily Briefing".to_string(),
        summary: "Existing".to_string(),
        trigger: "daily briefing".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a daily briefing workflow.".to_string(),
        confidence: 0.8,
        workflow_id: Some("wf-123".to_string()),
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("p_other".to_string()),
        last_error: None,
        snoozed_until: None,
        category: CATEGORY_WORK.to_string(),
        business_score: 0.78,
        feedback_status: None,
        feedback_note: None,
        feedback_count: 0,
        last_feedback_at: None,
    }];

    let decision = admit_auto_recommendation(&proposal, &existing);

    assert!(!decision.accepted);
    assert!(
        decision
            .reasons
            .iter()
            .any(|reason| reason.contains("history.approved_title_pending"))
            || decision
                .reasons
                .iter()
                .any(|reason| reason.contains("history.approved_fingerprint_pending"))
    );
}
