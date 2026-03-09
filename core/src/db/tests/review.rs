use super::*;

#[test]
#[serial]
fn test_recommendation_review_events_roundtrip() {
    init().ok();
    clear_recommendation_review_events_for_tests();

    record_recommendation_review_event(
        42,
        "Work Start Checklist",
        Some("approved"),
        Some("work"),
        "approve",
        Some("web_workflows"),
        Some("approved from workflows screen"),
        true,
        Some("workflow provisioning requested"),
    )
    .expect("record approved review event");
    record_recommendation_review_event(
        43,
        "Email Follow-Up Reminder",
        Some("pending"),
        Some("work"),
        "approve",
        Some("web_dashboard"),
        None,
        false,
        Some("needs more evidence before approval"),
    )
    .expect("record failed review event");

    let events = list_recommendation_review_events(10).expect("list recommendation review events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].recommendation_id, 43);
    assert_eq!(events[0].action, "approve");
    assert!(!events[0].ok);
    assert_eq!(events[0].status_after.as_deref(), Some("pending"));
    assert_eq!(events[1].actor.as_deref(), Some("web_workflows"));
    assert!(events[1].ok);
    assert_eq!(events[1].category.as_deref(), Some("work"));
}

#[test]
#[serial]
fn test_recommendation_review_metrics_aggregate_recent_events() {
    init().ok();
    clear_recommendation_review_events_for_tests();

    record_recommendation_review_event(
        1,
        "Work Start Checklist",
        Some("approved"),
        Some("work"),
        "approve",
        Some("web_workflows"),
        None,
        true,
        Some("approved"),
    )
    .expect("record approve");
    record_recommendation_review_event(
        2,
        "Email Follow-Up Reminder",
        Some("pending"),
        Some("work"),
        "later",
        Some("web_dashboard"),
        None,
        false,
        Some("cannot snooze recommendation"),
    )
    .expect("record later failure");
    record_recommendation_review_event(
        3,
        "Slack Digest",
        Some("rejected"),
        Some("work"),
        "feedback_negative",
        Some("web_dashboard"),
        Some("not useful"),
        true,
        Some("negative feedback"),
    )
    .expect("record negative feedback");
    record_recommendation_review_event(
        4,
        "Notion Summary",
        Some("pending"),
        Some("work"),
        "feedback_refine",
        Some("web_dashboard"),
        Some("notion only"),
        true,
        Some("refine feedback"),
    )
    .expect("record refine feedback");

    let metrics = get_recommendation_review_metrics(10).expect("review metrics");
    assert_eq!(metrics.total_events, 4);
    assert_eq!(metrics.approve_actions, 1);
    assert_eq!(metrics.later_actions, 1);
    assert_eq!(metrics.feedback_negative, 1);
    assert_eq!(metrics.feedback_refine, 1);
    assert_eq!(metrics.failed_actions, 1);
    assert_eq!(metrics.non_positive_feedback_rate, 100.0);
    assert!(metrics.action_failure_rate > 0.0);
}
