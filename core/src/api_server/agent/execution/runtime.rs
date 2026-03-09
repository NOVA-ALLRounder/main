use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{db, execution_controller, nl_store, verification_engine};

use super::support::{acquire_agent_execution, parse_resume_token};
use super::types::{AgentExecuteRequest, AgentExecutionContext, AgentExecutionRuntime};

pub(super) fn prepare_execution_context(
    payload: &AgentExecuteRequest,
) -> Result<AgentExecutionContext, Response> {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response());
    };
    let exec_guard = match acquire_agent_execution(&payload.plan_id) {
        Ok(g) => g,
        Err((scope, active_plan_id)) => {
            let error_code = if scope == "global" {
                "agent_execution_in_progress_global"
            } else {
                "plan_execution_in_progress"
            };
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": error_code,
                    "plan_id": payload.plan_id,
                    "lock_scope": scope,
                    "active_plan_id": active_plan_id,
                    "message": "다른 실행이 진행 중입니다. 현재 실행이 끝난 뒤 다시 시도하세요."
                })),
            )
                .into_response());
        }
    };

    let session = nl_store::find_session_by_plan(&payload.plan_id);
    let run_intent = session
        .as_ref()
        .map(|s| s.intent.intent.as_str().to_string())
        .unwrap_or_else(|| plan.intent.as_str().to_string());
    let run_prompt = session
        .as_ref()
        .map(|s| s.prompt.clone())
        .unwrap_or_else(|| format!("plan_id={}", payload.plan_id));
    let run_id = format!(
        "{}_{}",
        payload.plan_id,
        chrono::Utc::now().timestamp_millis()
    );

    match db::claim_task_run(&plan.plan_id, &run_id, &run_intent, &run_prompt, "running") {
        Ok(true) => {}
        Ok(false) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({
                    "error": "plan_execution_in_progress_db",
                    "plan_id": payload.plan_id
                })),
            )
                .into_response());
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "task_run_claim_failed",
                    "detail": e.to_string()
                })),
            )
                .into_response());
        }
    }

    let mut resume_from = payload
        .resume_from
        .unwrap_or_else(|| nl_store::get_plan_progress(&plan.plan_id).unwrap_or(0));
    let mut resume_hint = payload
        .resume_from
        .map(|idx| format!("resume_from_override={}", idx));

    if let Some(raw_token) = payload
        .resume_token
        .as_ref()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
    {
        match parse_resume_token(raw_token) {
            Ok(parsed) => {
                if parsed.plan_id != plan.plan_id {
                    crate::diagnostic_events::emit(
                        "agent.resume_token.invalid",
                        json!({
                            "reason": "plan_id_mismatch",
                            "request_plan_id": plan.plan_id,
                            "token_plan_id": parsed.plan_id
                        }),
                    );
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "error": "resume_token_plan_mismatch",
                            "plan_id": plan.plan_id
                        })),
                    )
                        .into_response());
                }
                resume_from = parsed.step_index;
                resume_hint = Some(format!(
                    "resume_token(reason={}, step={})",
                    parsed.reason, parsed.step_index
                ));
            }
            Err(err) => {
                crate::diagnostic_events::emit(
                    "agent.resume_token.invalid",
                    json!({
                        "reason": "parse_error",
                        "detail": err
                    }),
                );
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "resume_token_invalid",
                        "detail": err
                    })),
                )
                    .into_response());
            }
        }
    }

    if resume_from >= plan.steps.len() {
        if payload.resume_from.is_some() || payload.resume_token.is_some() {
            crate::diagnostic_events::emit(
                "agent.resume_token.invalid",
                json!({
                    "reason": "step_out_of_range",
                    "step_index": resume_from,
                    "plan_steps": plan.steps.len()
                }),
            );
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "resume_step_out_of_range",
                    "step_index": resume_from,
                    "plan_steps": plan.steps.len()
                })),
            )
                .into_response());
        }
        nl_store::clear_plan_progress(&plan.plan_id);
        resume_from = 0;
    }

    let execution_profile = payload.profile.unwrap_or_default();
    let execution_options = execution_profile.execution_options();

    Ok(AgentExecutionContext {
        plan,
        session,
        run_id,
        resume_from,
        resume_hint,
        execution_profile,
        execution_options,
        _exec_guard: exec_guard,
    })
}

pub(super) async fn execute_plan_with_runtime(
    context: &AgentExecutionContext,
) -> AgentExecutionRuntime {
    let mut result = execution_controller::execute_plan(
        &context.plan,
        context.resume_from,
        context.execution_options,
    )
    .await;
    super::super::contract::stamp_run_scope_evidence(
        &mut result.logs,
        &context.run_id,
        &context.plan.plan_id,
    );
    result.logs.push(format!(
        "Execution profile selected: {} (collision_policy={})",
        context.execution_profile.as_str(),
        context.execution_options.input_collision_policy.as_str()
    ));
    if let Some(hint) = &context.resume_hint {
        result.logs.push(format!("Resume hint: {}", hint));
    }
    if context.resume_from > 0 {
        result
            .logs
            .insert(0, format!("Resuming from step {}", context.resume_from + 1));
    }
    let mut verify = verification_engine::verify_execution(&context.plan, &result.logs);
    append_verification_log(&mut result.logs, &verify);

    let auto_replan_env = std::env::var("STEER_AUTO_REPLAN")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(true);
    let auto_replan = if context.execution_profile.default_auto_replan_enabled() {
        auto_replan_env
    } else {
        false
    };
    let allow_replan = !matches!(
        result.status.as_str(),
        "manual_required" | "approval_required" | "blocked"
    );

    if auto_replan && allow_replan && (result.status == "error" || !verify.ok) {
        result
            .logs
            .push("Auto-replan: retrying once after short wait".to_string());
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let retry =
            execution_controller::execute_plan(&context.plan, 0, context.execution_options).await;
        result.logs = retry.logs;
        super::super::contract::stamp_run_scope_evidence(
            &mut result.logs,
            &context.run_id,
            &context.plan.plan_id,
        );
        result.status = retry.status;
        result.approval = retry.approval;
        result.manual_steps = retry.manual_steps;
        result.resume_from = retry.resume_from;
        result.resume_token = retry.resume_token;
        verify = verification_engine::verify_execution(&context.plan, &result.logs);
        append_verification_log(&mut result.logs, &verify);
    }

    sync_plan_progress(&context.plan.plan_id, &result);

    AgentExecutionRuntime { result, verify }
}

fn append_verification_log(
    logs: &mut Vec<String>,
    verify: &crate::nl_automation::VerificationResult,
) {
    if !verify.ok {
        logs.push(format!("Verification failed: {}", verify.issues.join("; ")));
    } else {
        logs.push("Verification passed".to_string());
    }
}

fn sync_plan_progress(plan_id: &str, result: &crate::nl_automation::ExecutionResult) {
    if matches!(
        result.status.as_str(),
        "manual_required" | "approval_required"
    ) {
        if let Some(next_step) = result.resume_from {
            nl_store::set_plan_progress(plan_id, next_step);
        }
    } else {
        nl_store::clear_plan_progress(plan_id);
    }
}
