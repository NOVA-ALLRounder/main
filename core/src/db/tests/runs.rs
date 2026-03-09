use super::*;

#[test]
fn test_task_stage_retry_metadata_recorded() {
    init().ok();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    create_task_run(&run_id, "test", "retry metadata", "running").expect("create run");
    record_task_stage_run(&run_id, "execution", 2, "running", Some("start"))
        .expect("running stage");
    record_task_stage_run(&run_id, "execution", 2, "retrying", Some("retry attempt"))
        .expect("retrying stage");
    let stages = list_task_stage_runs(&run_id).expect("list stages");
    let latest = stages
        .into_iter()
        .rfind(|s| s.stage_name == "execution")
        .expect("latest execution stage");
    assert_eq!(latest.status, "retrying");
    assert!(latest.retry_count >= 1);
    assert!(latest.max_retries >= 0);
    assert!(latest.next_retry_at.is_some());
}

#[test]
fn test_task_stage_invalid_transition_rejected() {
    init().ok();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    create_task_run(&run_id, "test", "invalid transition", "running").expect("create run");
    record_task_stage_run(&run_id, "planner", 1, "running", Some("start"))
        .expect("planner running");
    record_task_stage_run(&run_id, "planner", 1, "completed", Some("done")).expect("planner done");
    let invalid = record_task_stage_run(&run_id, "planner", 1, "running", Some("should fail"));
    assert!(invalid.is_err());
}

#[test]
fn test_task_run_artifact_upsert_roundtrip() {
    init().ok();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    create_task_run(&run_id, "test", "artifact upsert", "running").expect("create run");

    upsert_task_run_artifact(
        &run_id,
        "artifact_assertion",
        "artifact.mail_sent_confirmed",
        "false",
        Some("{\"passed\":false}"),
    )
    .expect("insert artifact");
    upsert_task_run_artifact(
        &run_id,
        "artifact_assertion",
        "artifact.mail_sent_confirmed",
        "true",
        Some("{\"passed\":true}"),
    )
    .expect("update artifact");

    let artifacts = list_task_run_artifacts(&run_id).expect("list artifacts");
    let item = artifacts
        .into_iter()
        .find(|a| a.artifact_key == "artifact.mail_sent_confirmed")
        .expect("artifact row");
    assert_eq!(item.value, "true");
    assert_eq!(item.artifact_type, "artifact_assertion");
    assert!(item
        .metadata
        .unwrap_or_default()
        .contains("\"passed\":true"));
}
