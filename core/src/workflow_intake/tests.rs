use super::*;
use crate::db;
use crate::recommendation::AutomationProposal;
use serial_test::serial;

#[test]
#[serial]
fn manual_workflow_queue_applies_review_based_preferences() {
    db::init().ok();
    db::clear_recommendations_for_tests();

    let approved = AutomationProposal {
        title: "Notion Notes".to_string(),
        summary: "Save notes to Notion".to_string(),
        trigger: "manual notion".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.8,
        n8n_prompt: "Create a workflow that stores notes in Notion.".to_string(),
        evidence: vec![],
        pattern_id: Some("pref-approved".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.8,
    };
    let rejected = AutomationProposal {
        title: "Telegram Digest".to_string(),
        summary: "Send digest to Telegram".to_string(),
        trigger: "manual telegram".to_string(),
        actions: vec!["n8n Workflow".to_string()],
        confidence: 0.8,
        n8n_prompt: "Create a workflow that sends updates to Telegram.".to_string(),
        evidence: vec![],
        pattern_id: Some("pref-rejected".to_string()),
        category: crate::recommendation_policy::CATEGORY_WORK.to_string(),
        business_score: 0.8,
    };

    let (approved_id, _) =
        insert_or_get_recommendation_id(&approved).expect("insert approved preference rec");
    db::update_recommendation_review_status(approved_id, "approved")
        .expect("approve notion preference rec");

    let (rejected_id, _) =
        insert_or_get_recommendation_id(&rejected).expect("insert rejected preference rec");
    db::update_recommendation_review_status(rejected_id, "rejected")
        .expect("reject telegram preference rec");

    let outcome = queue_manual_workflow_recommendation(
        "회의 요약 자동화 만들어줘",
        "unit.test.manual_workflow",
    )
    .expect("queue manual workflow");
    let rec = db::get_recommendation(outcome.recommendation_id)
        .expect("get queued recommendation")
        .expect("queued recommendation row");

    assert!(rec.n8n_prompt.contains("Prefer Notion"));
    assert!(rec.n8n_prompt.contains("Avoid Telegram"));
}
