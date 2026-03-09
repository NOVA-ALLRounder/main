use crate::db;

use super::super::contract::{
    completion_score_pass_threshold, compute_completion_score, extract_summary, AgentStageDodCheck,
};
use super::types::{
    AgentExecutionBusinessState, AgentExecutionContext, AgentExecutionFinalization,
    AgentExecutionRuntime,
};

pub(super) fn finalize_execution(
    context: &AgentExecutionContext,
    runtime: &mut AgentExecutionRuntime,
    business: &AgentExecutionBusinessState,
    mut stage_dod: Vec<AgentStageDodCheck>,
) -> AgentExecutionFinalization {
    if runtime.result.status == "completed" && !business.business_complete {
        if !business.evidence_ok {
            runtime.result.logs.push(format!(
                "Business evidence failed: {}",
                business.evidence_detail
            ));
        }
        runtime.result.logs.push(
            "Final status downgraded: planner/execution complete but business completion failed"
                .to_string(),
        );
        runtime.result.status = "error".to_string();
    }

    let task_run_status = if business.business_complete {
        "business_completed"
    } else if matches!(
        runtime.result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    ) {
        "business_incomplete"
    } else {
        "business_failed"
    };

    let mut final_nl_status = if business.business_complete {
        "completed".to_string()
    } else if matches!(
        runtime.result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    ) {
        runtime.result.status.clone()
    } else {
        "error".to_string()
    };

    let mut completion_score = compute_completion_score(
        &final_nl_status,
        business.planner_complete,
        business.execution_complete,
        business.business_complete,
        business.verification_ok,
        business.evidence_ok,
        runtime.verify.issues.len(),
        runtime.result.manual_steps.len(),
    );
    if final_nl_status == "completed" && !completion_score.pass {
        runtime.result.logs.push(format!(
            "Final status downgraded: completion score below pass threshold (score={} threshold={})",
            completion_score.score,
            completion_score_pass_threshold()
        ));
        final_nl_status = "error".to_string();
        completion_score = compute_completion_score(
            &final_nl_status,
            business.planner_complete,
            business.execution_complete,
            business.business_complete,
            business.verification_ok,
            business.evidence_ok,
            runtime.verify.issues.len(),
            runtime.result.manual_steps.len(),
        );
    }

    let completion_expected = format!(">= {}", completion_score_pass_threshold());
    let completion_actual = completion_score.score.to_string();
    let completion_reasons = if completion_score.reasons.is_empty() {
        "none".to_string()
    } else {
        completion_score.reasons.join("; ")
    };
    let _ = db::record_task_stage_assertion(
        &context.run_id,
        "business",
        "business.completion_score",
        &completion_expected,
        &completion_actual,
        completion_score.pass,
        Some(&completion_reasons),
    );
    stage_dod.push(AgentStageDodCheck {
        stage: "business".to_string(),
        key: "business.completion_score".to_string(),
        expected: completion_expected,
        actual: completion_actual,
        passed: completion_score.pass,
        evidence: Some(completion_reasons.clone()),
    });
    runtime.result.logs.push(format!(
        "Completion score: {} ({}) pass={}",
        completion_score.score, completion_score.label, completion_score.pass
    ));

    let summary = extract_summary(&runtime.result.logs);
    let details_json = serde_json::to_string(&runtime.result.logs).unwrap_or_default();
    let _ = db::update_task_run_outcome(
        &context.run_id,
        business.planner_complete,
        business.execution_complete,
        business.business_complete,
        task_run_status,
        summary.as_deref(),
        Some(&details_json),
    );

    if let Some(state) = &context.session {
        let _ = db::insert_nl_run(
            state.intent.intent.as_str(),
            &state.prompt,
            &final_nl_status,
            summary.as_deref(),
            Some(&details_json),
        );
    }

    AgentExecutionFinalization {
        final_nl_status,
        completion_score,
        stage_dod,
    }
}
