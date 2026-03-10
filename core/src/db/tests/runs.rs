use super::*;
use serial_test::serial;

fn clear_task_runs_for_test() {
    let wrote = with_write_conn_if_available(|conn| {
        conn.execute("DELETE FROM task_runs", [])?;
        Ok(())
    })
    .expect("clear task runs write helper");
    assert!(wrote.is_some(), "db connection");
}

#[test]
#[serial]
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
#[serial]
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
#[serial]
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

#[test]
#[serial]
fn test_task_run_diagnostics_filters_and_stage_counts() {
    init().ok();
    let run_id = format!("run-{}", uuid::Uuid::new_v4());
    create_task_run(&run_id, "test", "diagnostic filters", "running").expect("create run");
    record_task_stage_run(&run_id, "planner", 1, "completed", Some("done")).expect("planner");
    record_task_stage_run(&run_id, "recovery", 2, "completed", Some("recovered"))
        .expect("recovery");

    record_task_stage_assertion(&run_id, "planner", "artifact.first", "ok", "ok", true, None)
        .expect("planner pass");
    record_task_stage_assertion(
        &run_id,
        "planner",
        "artifact.second",
        "ok",
        "fail",
        false,
        Some("planner failed"),
    )
    .expect("planner fail");
    record_task_stage_assertion(
        &run_id,
        "recovery",
        "recovery.focus",
        "ready",
        "ready",
        true,
        Some("recovered"),
    )
    .expect("recovery pass");

    upsert_task_run_artifact(&run_id, "log", "artifact.first", "v1", None).expect("artifact one");
    upsert_task_run_artifact(&run_id, "log", "artifact.second", "v2", None).expect("artifact two");

    let stages = list_task_stage_runs(&run_id).expect("list stages");
    let planner = stages
        .iter()
        .find(|stage| stage.stage_name == "planner")
        .expect("planner stage");
    assert_eq!(planner.assertion_total, 2);
    assert_eq!(planner.assertion_failed, 1);

    let failed_only = list_task_stage_assertions_with_options(
        &run_id,
        &TaskStageAssertionListOptions {
            failed_only: true,
            limit: Some(1),
            ..TaskStageAssertionListOptions::default()
        },
    )
    .expect("failed assertions");
    assert_eq!(failed_only.len(), 1);
    assert_eq!(failed_only[0].assertion_key, "artifact.second");

    let recovery_only = list_task_stage_assertions_with_options(
        &run_id,
        &TaskStageAssertionListOptions {
            stage_name: Some("recovery".to_string()),
            ..TaskStageAssertionListOptions::default()
        },
    )
    .expect("recovery assertions");
    assert_eq!(recovery_only.len(), 1);
    assert_eq!(recovery_only[0].stage_name, "recovery");

    let artifact_page = list_task_run_artifacts_with_options(
        &run_id,
        &TaskRunArtifactListOptions {
            artifact_type: Some("log".to_string()),
            limit: Some(1),
            ..TaskRunArtifactListOptions::default()
        },
    )
    .expect("artifact page");
    assert_eq!(artifact_page.len(), 1);
}

#[test]
#[serial]
fn test_claim_singleton_task_run_blocks_second_inflight_claim() {
    init().ok();
    clear_task_runs_for_test();

    let first_run_id = format!("singleton-{}", uuid::Uuid::new_v4());
    let second_run_id = format!("singleton-{}", uuid::Uuid::new_v4());

    let first_claim = claim_singleton_task_run(&first_run_id, "test", "singleton claim", "queued")
        .expect("first claim");
    let second_claim =
        claim_singleton_task_run(&second_run_id, "test", "singleton claim", "queued")
            .expect("second claim");

    assert!(first_claim);
    assert!(!second_claim);

    let active = get_latest_inflight_task_run()
        .expect("latest inflight lookup")
        .expect("active run");
    assert_eq!(active.run_id, first_run_id);

    clear_task_runs_for_test();
}
