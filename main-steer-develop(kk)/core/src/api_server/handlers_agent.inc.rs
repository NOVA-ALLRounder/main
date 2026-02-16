pub(super) async fn agent_intent_handler(
    Json(payload): Json<AgentIntentRequest>,
) -> impl IntoResponse {
    let intent_result = intent_router::classify_intent(&payload.text);
    let fill = slot_filler::fill_slots(&intent_result.intent, intent_result.slots.clone());
    let session = nl_store::create_session(intent_result.clone(), fill.slots.clone(), payload.text.clone());

    let response = AgentIntentResponse {
        session_id: session.session_id,
        intent: intent_result.intent.as_str().to_string(),
        confidence: intent_result.confidence,
        slots: fill.slots,
        missing_slots: fill.missing,
        follow_up: fill.follow_up,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn agent_plan_handler(
    Json(payload): Json<AgentPlanRequest>,
) -> impl IntoResponse {
    let Some(mut session) = nl_store::get_session(&payload.session_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "session_not_found" })),
        )
            .into_response();
    };

    if let Some(updates) = payload.slots.as_ref() {
        if let Some(updated) = nl_store::update_session_slots(&payload.session_id, updates) {
            session = updated;
        }
    }

    let fill = slot_filler::fill_slots(&session.intent.intent, session.slots.clone());
    let plan = plan_builder::build_plan(&session.intent.intent, &fill.slots);
    let _ = nl_store::set_session_plan(&payload.session_id, plan.clone());

    let response = AgentPlanResponse {
        plan_id: plan.plan_id,
        intent: session.intent.intent.as_str().to_string(),
        steps: plan.steps,
        missing_slots: fill.missing,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn agent_execute_handler(
    Json(payload): Json<AgentExecuteRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    let mut resume_from = nl_store::get_plan_progress(&plan.plan_id).unwrap_or(0);
    if resume_from >= plan.steps.len() {
        nl_store::clear_plan_progress(&plan.plan_id);
        resume_from = 0;
    }
    let mut result = execution_controller::execute_plan(&plan, resume_from).await;
    if resume_from > 0 {
        result
            .logs
            .insert(0, format!("Resuming from step {}", resume_from + 1));
    }
    let mut verify = verification_engine::verify_execution(&plan, &result.logs);
    if !verify.ok {
        result.logs.push(format!("Verification failed: {}", verify.issues.join("; ")));
    } else {
        result.logs.push("Verification passed".to_string());
    }

    // Simple auto-replan: one retry on failure or verification issues
    let auto_replan = std::env::var("STEER_AUTO_REPLAN")
        .ok()
        .map(|v| matches!(v.trim().to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(true);
    let allow_replan = !matches!(result.status.as_str(), "manual_required" | "approval_required");
    if auto_replan && allow_replan && (result.status == "error" || !verify.ok) {
        result.logs.push("Auto-replan: retrying once after short wait".to_string());
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        let retry = execution_controller::execute_plan(&plan, 0).await;
        result.logs = retry.logs;
        result.status = retry.status;
        result.approval = retry.approval;
        result.manual_steps = retry.manual_steps;
        result.resume_from = retry.resume_from;
        verify = verification_engine::verify_execution(&plan, &result.logs);
        if !verify.ok {
            result
                .logs
                .push(format!("Verification failed: {}", verify.issues.join("; ")));
        } else {
            result.logs.push("Verification passed".to_string());
        }
    }

    if matches!(result.status.as_str(), "manual_required" | "approval_required") {
        if let Some(next_step) = result.resume_from {
            nl_store::set_plan_progress(&plan.plan_id, next_step);
        }
    } else {
        nl_store::clear_plan_progress(&plan.plan_id);
    }
    let session = nl_store::find_session_by_plan(&payload.plan_id);
    let summary = extract_summary(&result.logs);
    if let Some(state) = session {
        let _ = db::insert_nl_run(
            state.intent.intent.as_str(),
            &state.prompt,
            &result.status,
            summary.as_deref(),
            Some(&serde_json::to_string(&result.logs).unwrap_or_default()),
        );
    }
    let response = AgentExecuteResponse {
        status: result.status,
        logs: result.logs,
        approval: result.approval,
        manual_steps: result.manual_steps,
        resume_from: result.resume_from,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn agent_verify_handler(
    Json(payload): Json<AgentVerifyRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    let result = verification_engine::verify_plan(&plan);
    let response = AgentVerifyResponse {
        ok: result.ok,
        issues: result.issues,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) async fn agent_approve_handler(
    Json(payload): Json<AgentApproveRequest>,
) -> impl IntoResponse {
    let Some(plan) = nl_store::get_plan(&payload.plan_id) else {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "plan_not_found" })),
        )
            .into_response();
    };

    if let Some(decision) = payload.decision.as_deref() {
        approval_gate::register_decision(decision, &payload.action, &plan);
    }
    let decision = approval_gate::preview_approval(&payload.action, &plan);
    let response = AgentApproveResponse {
        status: decision.status,
        requires_approval: decision.requires_approval,
        message: decision.message,
        risk_level: decision.risk_level,
        policy: decision.policy,
    };

    (StatusCode::OK, Json(response)).into_response()
}

pub(super) fn extract_summary(logs: &[String]) -> Option<String> {
    logs.iter()
        .find_map(|line| line.strip_prefix("Summary: ").map(|s| s.to_string()))
}

