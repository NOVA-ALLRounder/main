use super::*;
use crate::recommendation::AutomationProposal;
use serial_test::serial;

fn insert_test_recommendation(title: &str) -> Option<i64> {
    let _ = db::init();
    let proposal = AutomationProposal {
        title: title.to_string(),
        summary: "test summary".to_string(),
        trigger: format!("trigger-{}", title),
        actions: vec!["noop".to_string()],
        confidence: 0.9,
        n8n_prompt: format!("Create workflow {}", title),
        evidence: vec!["test".to_string()],
        pattern_id: None,
        category: crate::recommendation_policy::CATEGORY_UNKNOWN.to_string(),
        business_score: 0.0,
    };
    let _ = db::insert_recommendation(&proposal);
    db::get_recommendations_with_filter(Some("pending"))
        .ok()
        .and_then(|recs| recs.into_iter().find(|r| r.title == title))
        .map(|r| r.id)
}

fn insert_test_auto_recommendation(
    title: &str,
    confidence: f64,
    business_score: f64,
    evidence: Vec<String>,
) -> Option<i64> {
    let _ = db::init();
    let proposal = AutomationProposal {
        title: title.to_string(),
        summary: "Detected 3 repeats across 1 distinct day(s).".to_string(),
        trigger: format!("trigger-{}", title),
        actions: vec!["noop".to_string()],
        confidence,
        n8n_prompt: format!("Create workflow {}", title),
        evidence,
        pattern_id: Some(format!("pattern-{}", title)),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score,
    };
    let _ = db::insert_recommendation(&proposal);
    db::get_recommendations_with_filter(Some("pending"))
        .ok()
        .and_then(|recs| recs.into_iter().find(|r| r.title == title))
        .map(|r| r.id)
}

#[test]
#[serial]
fn test_assume_approved_requires_flag() {
    std::env::remove_var("STEER_TEST_ASSUME_APPROVED");
    let result = maybe_assume_approved_for_test(1);
    assert!(result.is_err());
}

#[test]
#[serial]
fn test_assume_approved_requires_mock_flag() {
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::remove_var("STEER_N8N_MOCK");
    let result = maybe_assume_approved_for_test(1);
    assert!(result.is_err());
}

#[test]
fn workflow_generation_prompt_includes_refinement_feedback() {
    let rec = db::Recommendation {
        id: 1,
        status: "pending".to_string(),
        title: "Meeting Prep Assistant".to_string(),
        summary: "summary".to_string(),
        trigger: "trigger".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        n8n_prompt: "Create a meeting prep workflow.".to_string(),
        confidence: 0.8,
        workflow_id: None,
        workflow_json: None,
        evidence: vec![],
        pattern_id: Some("pattern".to_string()),
        last_error: None,
        snoozed_until: None,
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.8,
        feedback_status: Some("refine".to_string()),
        feedback_note: Some("텔레그램 말고 노션에 저장해줘".to_string()),
        feedback_count: 1,
        last_feedback_at: Some(chrono::Utc::now().to_rfc3339()),
    };

    let prompt = workflow_generation_prompt(&rec);
    assert!(prompt.contains("Create a meeting prep workflow."));
    assert!(prompt.contains("텔레그램 말고 노션에 저장해줘"));
}

#[tokio::test]
#[serial]
async fn test_execute_requires_approved_status() {
    let title = format!(
        "rec-exec-requires-{}",
        chrono::Utc::now().timestamp_millis()
    );
    let Some(id) = insert_test_recommendation(&title) else {
        return;
    };
    std::env::remove_var("STEER_TEST_ASSUME_APPROVED");
    std::env::set_var("STEER_N8N_MOCK", "1");
    let res = execute_approved_recommendation(id, None).await;
    assert!(res.is_err());
}

#[tokio::test]
#[serial]
async fn test_approve_assumed_pipeline_with_mock() {
    let title = format!(
        "rec-approve-assumed-{}",
        chrono::Utc::now().timestamp_millis()
    );
    let Some(id) = insert_test_recommendation(&title) else {
        return;
    };
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::set_var("STEER_N8N_MOCK", "1");

    assert!(maybe_assume_approved_for_test(id).is_ok());
    let workflow_id = execute_approved_recommendation(id, None)
        .await
        .expect("approve-assumed mock path should return workflow_id");
    assert!(!workflow_id.trim().is_empty());

    let rec = db::get_recommendation(id).ok().flatten();
    assert!(rec.is_some());
    let rec = rec.unwrap();
    assert_eq!(rec.status, "approved");
    assert!(rec.workflow_id.unwrap_or_default().starts_with("mock-wf-"));
}

#[tokio::test]
#[serial]
async fn test_execute_is_idempotent_for_existing_workflow() {
    let title = format!("rec-idempotent-{}", chrono::Utc::now().timestamp_millis());
    let Some(id) = insert_test_recommendation(&title) else {
        return;
    };
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::set_var("STEER_N8N_MOCK", "1");
    std::env::remove_var("STEER_APPROVE_FORCE_RECREATE");

    assert!(maybe_assume_approved_for_test(id).is_ok());
    let first = execute_approved_recommendation(id, None)
        .await
        .expect("first execution should provision workflow");
    assert!(!first.is_empty());

    let second = execute_approved_recommendation(id, None)
        .await
        .expect("second execution should reuse existing workflow");
    assert_eq!(first, second);
}

#[tokio::test]
#[serial]
async fn test_stale_provisioning_claim_is_recovered() {
    let title = format!("rec-stale-claim-{}", chrono::Utc::now().timestamp_millis());
    let Some(id) = insert_test_recommendation(&title) else {
        return;
    };
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::set_var("STEER_N8N_MOCK", "1");
    std::env::set_var("STEER_PROVISIONING_CLAIM_TTL_SECONDS", "1");
    std::env::remove_var("STEER_APPROVE_FORCE_RECREATE");
    assert!(maybe_assume_approved_for_test(id).is_ok());

    let stale_token = format!("provisioning:{}:1", id);
    let _ = db::claim_recommendation_provisioning(id, &stale_token);
    let workflow_id = execute_approved_recommendation(id, None)
        .await
        .expect("stale provisioning claim should be recovered");
    assert!(!workflow_id.trim().is_empty());
    assert!(!workflow_id.starts_with("provisioning:"));
}

#[tokio::test]
#[serial]
async fn test_approve_and_execute_reports_status() {
    let title = format!("rec-approve-exec-{}", chrono::Utc::now().timestamp_millis());
    let Some(id) = insert_test_recommendation(&title) else {
        return;
    };
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::set_var("STEER_N8N_MOCK", "1");
    std::env::remove_var("STEER_APPROVE_FORCE_RECREATE");

    let first = approve_and_execute_recommendation(id, None)
        .await
        .expect("first approve-and-execute should provision workflow");
    assert!(!first.workflow_id.trim().is_empty());
    assert!(first.approved_now);

    let second = approve_and_execute_recommendation(id, None)
        .await
        .expect("second approve-and-execute should reuse workflow");
    assert_eq!(first.workflow_id, second.workflow_id);
    assert!(!second.approved_now);
    assert!(second.reused_existing);
}

#[tokio::test]
#[serial]
async fn test_auto_recommendation_requires_readiness_before_approval() {
    db::clear_recommendations_for_tests();
    let title = format!("rec-auto-gate-{}", chrono::Utc::now().timestamp_millis());
    let Some(id) = insert_test_auto_recommendation(
        &title,
        0.74,
        0.61,
        vec![
            "Frequency: Found 3 occurrences".to_string(),
            "Span: 1 distinct day(s)".to_string(),
            "policy.reason=support_work_signals=terminal,vscode".to_string(),
        ],
    ) else {
        return;
    };
    std::env::set_var("STEER_TEST_ASSUME_APPROVED", "1");
    std::env::set_var("STEER_N8N_MOCK", "1");

    let result = approve_and_execute_recommendation(id, None).await;

    assert!(result.is_err());
    let message = result.err().unwrap().to_string();
    assert!(message.contains("not ready for approval"));

    let rec = db::get_recommendation(id)
        .expect("load rec")
        .expect("existing rec");
    assert_eq!(rec.status, "pending");
}

#[test]
fn execution_status_classifier_matches_expected_values() {
    assert!(is_execution_success_status("success"));
    assert!(is_execution_success_status("completed"));
    assert!(!is_execution_success_status("running"));

    assert!(is_execution_failure_status("failed"));
    assert!(is_execution_failure_status("error"));
    assert!(is_execution_failure_status("cancelled"));
    assert!(!is_execution_failure_status("running"));
}
