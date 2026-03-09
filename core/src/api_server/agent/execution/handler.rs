use axum::{http::StatusCode, response::IntoResponse, Json};

use super::finalize::finalize_execution;
use super::recording::record_stage_dod;
use super::runtime::{execute_plan_with_runtime, prepare_execution_context};
use super::types::{AgentExecuteRequest, AgentExecuteResponse};

pub(crate) async fn agent_execute_handler(
    Json(payload): Json<AgentExecuteRequest>,
) -> impl IntoResponse {
    let context = match prepare_execution_context(&payload) {
        Ok(context) => context,
        Err(response) => return response,
    };
    let mut runtime = execute_plan_with_runtime(&context).await;
    let (evidence_ok, evidence_detail) =
        super::super::contract::evaluate_business_evidence(&context.plan, &runtime.result.logs);
    let business = super::types::AgentExecutionBusinessState {
        planner_complete: !context.plan.steps.is_empty(),
        execution_complete: runtime.result.status == "completed",
        verification_ok: runtime.verify.ok,
        evidence_ok,
        evidence_detail,
        business_complete: !context.plan.steps.is_empty()
            && runtime.result.status == "completed"
            && runtime.verify.ok
            && evidence_ok,
    };
    let stage_dod = record_stage_dod(&context, &runtime, &business);
    let finalized = finalize_execution(&context, &mut runtime, &business, stage_dod);

    let response = AgentExecuteResponse {
        status: finalized.final_nl_status,
        logs: runtime.result.logs,
        approval: runtime.result.approval,
        manual_steps: runtime.result.manual_steps,
        resume_from: runtime.result.resume_from,
        resume_token: runtime.result.resume_token,
        run_id: Some(context.run_id),
        planner_complete: business.planner_complete,
        execution_complete: business.execution_complete,
        business_complete: business.business_complete,
        completion_score: Some(finalized.completion_score),
        profile: Some(context.execution_profile.as_str().to_string()),
        collision_policy: Some(
            context
                .execution_options
                .input_collision_policy
                .as_str()
                .to_string(),
        ),
        stage_dod: finalized.stage_dod,
    };

    (StatusCode::OK, Json(response)).into_response()
}
