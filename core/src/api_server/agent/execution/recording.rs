use serde_json::json;

use crate::db;

use super::super::contract::{detect_artifact_evidence_assertions, AgentStageDodCheck};
use super::types::{AgentExecutionBusinessState, AgentExecutionContext, AgentExecutionRuntime};

pub(super) fn record_stage_dod(
    context: &AgentExecutionContext,
    runtime: &AgentExecutionRuntime,
    business: &AgentExecutionBusinessState,
) -> Vec<AgentStageDodCheck> {
    let planner_actual = business.planner_complete.to_string();
    let execution_actual = business.execution_complete.to_string();
    let verify_actual = business.verification_ok.to_string();
    let evidence_actual = business.evidence_ok.to_string();
    let business_actual = business.business_complete.to_string();
    let mut stage_dod = Vec::new();

    let _ = db::record_task_stage_run(
        &context.run_id,
        "planner",
        1,
        "running",
        Some("planner outcome evaluation"),
    );
    let _ = db::record_task_stage_run(
        &context.run_id,
        "planner",
        1,
        if business.planner_complete {
            "completed"
        } else {
            "failed"
        },
        Some(&format!("plan_steps={}", context.plan.steps.len())),
    );
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "planner",
        "planner.plan_steps_non_empty",
        "true",
        &planner_actual,
        business.planner_complete,
        Some(&format!("plan_id={}", context.plan.plan_id)),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "planner".to_string(),
        key: "planner.plan_steps_non_empty".to_string(),
        expected: "true".to_string(),
        actual: planner_actual,
        passed: business.planner_complete,
        evidence: Some(format!("plan_id={}", context.plan.plan_id)),
    });

    let _ = db::record_task_stage_run(
        &context.run_id,
        "execution",
        2,
        "running",
        Some("execution result evaluation"),
    );
    let _ = db::record_task_stage_run(
        &context.run_id,
        "execution",
        2,
        if business.execution_complete {
            "completed"
        } else {
            runtime.result.status.as_str()
        },
        Some(&format!(
            "manual_steps={} logs={}",
            runtime.result.manual_steps.len(),
            runtime.result.logs.len()
        )),
    );
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "execution",
        "execution.status_completed",
        "true",
        &execution_actual,
        business.execution_complete,
        Some(runtime.result.status.as_str()),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "execution".to_string(),
        key: "execution.status_completed".to_string(),
        expected: "true".to_string(),
        actual: execution_actual,
        passed: business.execution_complete,
        evidence: Some(runtime.result.status.clone()),
    });

    let _ = db::record_task_stage_run(
        &context.run_id,
        "verification",
        3,
        "running",
        Some("verification result evaluation"),
    );
    let _ = db::record_task_stage_run(
        &context.run_id,
        "verification",
        3,
        if business.verification_ok {
            "completed"
        } else {
            "failed"
        },
        Some(&format!("issues={}", runtime.verify.issues.join("; "))),
    );
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "verification",
        "verification.verify_ok",
        "true",
        &verify_actual,
        business.verification_ok,
        Some(&runtime.verify.issues.join("; ")),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "verification".to_string(),
        key: "verification.verify_ok".to_string(),
        expected: "true".to_string(),
        actual: verify_actual,
        passed: business.verification_ok,
        evidence: Some(runtime.verify.issues.join("; ")),
    });

    let business_stage_requirements =
        "requires planner_complete && execution_complete && verify_ok && business_evidence_ok";
    let business_stage_details = format!(
        "{}; evidence={}",
        business_stage_requirements, business.evidence_detail
    );
    let _ = db::record_task_stage_run(
        &context.run_id,
        "business",
        4,
        "running",
        Some("business evidence evaluation"),
    );
    let _ = db::record_task_stage_run(
        &context.run_id,
        "business",
        4,
        if business.business_complete {
            "completed"
        } else {
            "failed"
        },
        Some(&business_stage_details),
    );
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "business",
        "business.business_evidence_ok",
        "true",
        &evidence_actual,
        business.evidence_ok,
        Some(&business.evidence_detail),
    );
    let _ = db::upsert_task_run_artifact(
        &context.run_id,
        "business",
        "business.business_evidence_ok",
        &evidence_actual,
        Some(&business.evidence_detail),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.business_evidence_ok".to_string(),
        expected: "true".to_string(),
        actual: evidence_actual,
        passed: business.evidence_ok,
        evidence: Some(business.evidence_detail.clone()),
    });
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "business",
        "business.business_complete",
        "true",
        &business_actual,
        business.business_complete,
        Some(business_stage_requirements),
    );
    let _ = db::upsert_task_run_artifact(
        &context.run_id,
        "business",
        "business.business_complete",
        &business_actual,
        Some(business_stage_requirements),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.business_complete".to_string(),
        expected: "true".to_string(),
        actual: business_actual,
        passed: business.business_complete,
        evidence: Some(business_stage_requirements.to_string()),
    });

    for assertion in detect_artifact_evidence_assertions(&context.plan, &runtime.result.logs) {
        let _ = db::record_task_stage_assertion(
            &context.run_id,
            "business",
            assertion.key,
            assertion.expected.as_str(),
            assertion.actual.as_str(),
            assertion.passed,
            Some(assertion.evidence.as_str()),
        );
        let metadata = json!({
            "expected": assertion.expected.as_str(),
            "passed": assertion.passed,
            "evidence": assertion.evidence.as_str()
        })
        .to_string();
        let _ = db::upsert_task_run_artifact(
            &context.run_id,
            "artifact_assertion",
            assertion.key,
            assertion.actual.as_str(),
            Some(metadata.as_str()),
        );
        stage_dod.push(AgentStageDodCheck {
            stage: "business".to_string(),
            key: assertion.key.to_string(),
            expected: assertion.expected.to_string(),
            actual: assertion.actual.to_string(),
            passed: assertion.passed,
            evidence: Some(assertion.evidence.to_string()),
        });
    }

    stage_dod
}
