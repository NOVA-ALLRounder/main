use super::*;

pub async fn execute_plan(
    plan: &Plan,
    start_index: usize,
    options: ExecutionOptions,
) -> ExecutionResult {
    let mut logs = Vec::new();
    let mut manual_required = false;
    let mut manual_steps: Vec<String> = Vec::new();
    let mut approval_required = false;
    let mut blocked = false;
    let mut blocked_reason = "policy_blocked".to_string();
    let mut approval_context: Option<ApprovalContext> = None;
    let mut resume_from: Option<usize> = None;
    let mut focus_handoff_state = FocusHandoffState::default();

    logs.push(format!(
        "Start plan {} ({})",
        plan.plan_id,
        plan.intent.as_str()
    ));
    logs.push(summary_for_plan(plan));
    logs.push(format!(
        "Execution options: enforce_browser_focus={}, input_collision_policy={}",
        options.enforce_browser_focus,
        options.input_collision_policy.as_str()
    ));
    push_run_attempt(
        &mut logs,
        "execution_start",
        "running",
        &format!("plan_id={},start_index={}", plan.plan_id, start_index),
    );

    for (idx, step) in plan.steps.iter().enumerate().skip(start_index) {
        logs.push(format!(
            "Step {}: {} ({:?})",
            idx + 1,
            step.description,
            step.step_type
        ));

        if interrupt_guard_enabled()
            && options.input_collision_policy != InputCollisionPolicy::Ignore
            && should_guard_interrupt_for_step(&step.step_type)
        {
            if let Some(expected_app) = expected_front_app_for_step(&step.data) {
                let front_app = current_platform()
                    .frontmost_app_name()
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let mut front_trimmed = front_app.trim().to_string();
                let mut recovered = false;
                if !front_trimmed.is_empty() && !app_matches_expected(&front_trimmed, &expected_app)
                {
                    let _ = wait_until_user_idle_if_active(
                        &mut logs,
                        idx,
                        "frontmost_mismatch_expected_app",
                    )
                    .await;
                    let front_after_idle = current_platform()
                        .frontmost_app_name()
                        .ok()
                        .flatten()
                        .unwrap_or_default();
                    if !front_after_idle.trim().is_empty() {
                        front_trimmed = front_after_idle.trim().to_string();
                    }
                    if focus_handoff_enabled() {
                        let (ok, recovered_front) = recover_expected_focus(
                            &mut logs,
                            &mut focus_handoff_state,
                            idx,
                            &expected_app,
                            &front_trimmed,
                        )
                        .await;
                        recovered = ok;
                        if !recovered_front.trim().is_empty() {
                            front_trimmed = recovered_front;
                        }
                    }
                }
                if !front_trimmed.is_empty()
                    && !app_matches_expected(&front_trimmed, &expected_app)
                    && !recovered
                {
                    manual_required = true;
                    resume_from = Some(idx);
                    manual_steps.push(format!(
                        "Step {} 전면 앱 충돌: expected={} actual={} (수동 복구 후 Resume)",
                        idx + 1,
                        expected_app,
                        front_trimmed
                    ));
                    logs.push(format!(
                        "INTERRUPT_DETECTED: step={} expected_app={} frontmost={} policy={}",
                        idx + 1,
                        expected_app,
                        front_trimmed,
                        options.input_collision_policy.as_str()
                    ));
                    push_run_attempt(
                        &mut logs,
                        "user_interrupt",
                        "manual_required",
                        &format!(
                            "step={} expected_app={} frontmost={}",
                            idx + 1,
                            expected_app,
                            front_trimmed
                        ),
                    );
                    break;
                } else if recovered {
                    logs.push(format!(
                        "FOCUS_HANDOFF_RECOVERED: step={} expected_app={} frontmost={}",
                        idx + 1,
                        expected_app,
                        front_trimmed
                    ));
                }
            }
        }

        if options.enforce_browser_focus && step_requires_browser_focus(&step.step_type) {
            let mut front_app = current_platform()
                .frontmost_app_name()
                .ok()
                .flatten()
                .unwrap_or_default();
            if !is_browser_app(&front_app) && focus_handoff_enabled() {
                let _ =
                    wait_until_user_idle_if_active(&mut logs, idx, "browser_focus_required").await;
                let front_after_idle = current_platform()
                    .frontmost_app_name()
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                if !front_after_idle.trim().is_empty() {
                    front_app = front_after_idle;
                }
                let (ok, recovered_front) =
                    recover_browser_focus(&mut logs, &mut focus_handoff_state, idx, &front_app)
                        .await;
                if ok {
                    front_app = recovered_front;
                }
            }
            if !is_browser_app(&front_app) {
                let collision_details = format!(
                    "step={} expected=browser frontmost={} policy={}",
                    idx + 1,
                    front_app,
                    options.input_collision_policy.as_str()
                );
                logs.push(format!(
                    "INPUT_COLLISION: UI step requires browser focus but frontmost app is '{}'",
                    front_app
                ));
                push_run_attempt(&mut logs, "input_collision", "detected", &collision_details);
                match options.input_collision_policy {
                    InputCollisionPolicy::Ignore => {
                        logs.push("Input collision ignored by execution profile".to_string());
                    }
                    InputCollisionPolicy::Pause => {
                        manual_required = true;
                        resume_from = Some(idx);
                        manual_steps.push(format!(
                            "Step {} 실행 전 브라우저를 전면으로 복구하고 Resume 하세요",
                            idx + 1
                        ));
                        logs.push(
                            "Execution paused due to input collision (manual intervention required)"
                                .to_string(),
                        );
                        break;
                    }
                    InputCollisionPolicy::Abort => {
                        blocked = true;
                        blocked_reason = "input_collision_abort".to_string();
                        resume_from = Some(idx);
                        logs.push(
                            "Execution aborted due to input collision policy (strict)".to_string(),
                        );
                        break;
                    }
                }
            }
        }

        match step.step_type {
            StepType::Navigate => {
                if let Some(url) = step.data.get("url").and_then(|v| v.as_str()) {
                    if let Err(err) = browser_automation::open_url_in_chrome(url)
                        .or_else(|_| crate::applescript::open_url(url).map(|_| ()))
                    {
                        logs.push(format!("Failed to open url {}: {}", url, err));
                        return ExecutionResult {
                            status: "error".to_string(),
                            logs,
                            approval: approval_context,
                            manual_steps,
                            resume_from,
                            resume_token: None,
                        };
                    }
                } else {
                    logs.push("Navigate step missing url".to_string());
                }
            }
            StepType::Wait => {
                let seconds = step
                    .data
                    .get("seconds")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1);
                tokio::time::sleep(tokio::time::Duration::from_secs(seconds)).await;
            }
            StepType::Select => {
                if is_auto_step(&step.data) {
                    let applied = match plan.intent {
                        crate::nl_automation::IntentType::FlightSearch => {
                            let budget = step.data.get("budget").and_then(|v| v.as_str());
                            let time_window = step.data.get("time_window").and_then(|v| v.as_str());
                            let direct_only = step.data.get("direct_only").and_then(|v| v.as_str());
                            if budget.is_none() && time_window.is_none() && direct_only.is_none() {
                                logs.push("No flight filters to apply".to_string());
                                continue;
                            }
                            browser_automation::apply_flight_filters(
                                budget,
                                time_window,
                                direct_only,
                            )
                        }
                        crate::nl_automation::IntentType::ShoppingCompare => {
                            let brand = step.data.get("brand").and_then(|v| v.as_str());
                            let price_min = step.data.get("price_min").and_then(|v| v.as_str());
                            let price_max = step.data.get("price_max").and_then(|v| v.as_str());
                            if brand.is_none() && price_min.is_none() && price_max.is_none() {
                                logs.push("No shopping filters to apply".to_string());
                                continue;
                            }
                            browser_automation::apply_shopping_filters(brand, price_min, price_max)
                        }
                        _ => Ok(false),
                    };

                    match applied {
                        Ok(true) => logs.push("Filters applied".to_string()),
                        Ok(false) => {
                            manual_required = true;
                            manual_steps.push(step.description.clone());
                            logs.push(format!(
                                "Manual filters required for step '{}'",
                                step.description
                            ));
                        }
                        Err(err) => {
                            manual_required = true;
                            manual_steps.push(step.description.clone());
                            logs.push(format!("Filter apply failed: {}", err));
                        }
                    }
                } else {
                    manual_required = true;
                    manual_steps.push(step.description.clone());
                    logs.push(format!(
                        "Manual filters required for step '{}'",
                        step.description
                    ));
                }
            }
            StepType::Fill | StepType::Click => {
                if is_auto_step(&step.data) {
                    if let Some(action) = step.data.get("action").and_then(|v| v.as_str()) {
                        if action == "submit_search" {
                            let mut clicked = false;
                            for attempt in 0..2 {
                                match browser_automation::click_search_button() {
                                    Ok(true) => {
                                        logs.push("Clicked search button".to_string());
                                        clicked = true;
                                        break;
                                    }
                                    Ok(false) => {
                                        logs.push(format!(
                                            "Search button not found (attempt {})",
                                            attempt + 1
                                        ));
                                        if attempt == 0 {
                                            let _ = browser_automation::scroll_page(600);
                                            tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                                .await;
                                        }
                                    }
                                    Err(err) => {
                                        logs.push(format!("Search click failed: {}", err));
                                        if attempt == 0 {
                                            let _ = browser_automation::scroll_page(600);
                                            tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                                .await;
                                        }
                                    }
                                }
                            }
                            if !clicked {
                                if let Ok(ctx) = browser_automation::get_page_context() {
                                    logs.push(format!("Page context: {}", ctx));
                                }
                                manual_required = true;
                                manual_steps.push(step.description.clone());
                            }
                            continue;
                        }
                    }
                    if let Some(field) = step.data.get("field").and_then(|v| v.as_str()) {
                        let mut filled = false;
                        for attempt in 0..2 {
                            match try_browser_autofill(plan, field) {
                                Ok(true) => {
                                    logs.push(format!("Auto fill succeeded for {}", field));
                                    filled = true;
                                    break;
                                }
                                Ok(false) => {
                                    logs.push(format!(
                                        "Auto fill skipped (no match) for {} (attempt {})",
                                        field,
                                        attempt + 1
                                    ));
                                    if attempt == 0 {
                                        let _ = browser_automation::scroll_page(400);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                            .await;
                                    }
                                }
                                Err(err) => {
                                    logs.push(format!(
                                        "Auto fill failed: {} (attempt {})",
                                        err,
                                        attempt + 1
                                    ));
                                    if attempt == 0 {
                                        let _ = browser_automation::scroll_page(400);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                            .await;
                                    }
                                }
                            }
                        }
                        if !filled {
                            if let Ok(ctx) = browser_automation::get_page_context() {
                                logs.push(format!("Page context: {}", ctx));
                            }
                            manual_required = true;
                            manual_steps.push(step.description.clone());
                        }
                        continue;
                    }
                    if let Some(value) = step.data.get("value").and_then(|v| v.as_str()) {
                        let mut driver = VisualDriver::new();
                        driver.add_step(SmartStep::new(
                            UiAction::Type(value.to_string()),
                            "Type value",
                        ));
                        if let Err(err) = driver.execute(None).await {
                            logs.push(format!("Auto input failed: {}", err));
                            manual_required = true;
                            manual_steps.push(step.description.clone());
                        } else {
                            logs.push("Auto input attempted".to_string());
                        }
                    } else if let Some(query) = step.data.get("query").and_then(|v| v.as_str()) {
                        let mut driver = VisualDriver::new();
                        driver.add_step(SmartStep::new(
                            UiAction::Type(query.to_string()),
                            "Type query",
                        ));
                        if let Err(err) = driver.execute(None).await {
                            logs.push(format!("Auto input failed: {}", err));
                            manual_required = true;
                            manual_steps.push(step.description.clone());
                        } else {
                            logs.push("Auto input attempted".to_string());
                        }
                    } else {
                        manual_required = true;
                        manual_steps.push(step.description.clone());
                        logs.push(format!(
                            "Manual input required for step '{}'",
                            step.description
                        ));
                    }
                } else {
                    manual_required = true;
                    manual_steps.push(step.description.clone());
                    logs.push(format!(
                        "Manual input required for step '{}'",
                        step.description
                    ));
                }
            }
            StepType::Approve => {
                let action = step
                    .data
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("approve");
                let decision = approval_gate::evaluate_approval(action, plan);
                let approval_id = format!("appr:{}:{}:{}", plan.plan_id, step.step_id, idx + 1);
                logs.push(format!(
                    "Approval check: {} (risk {}, policy {})",
                    decision.status, decision.risk_level, decision.policy
                ));
                logs.push(format!(
                    "APPROVAL_CHECKPOINT|approval_id={}|step_id={}|status={}|risk={}|policy={}",
                    approval_id,
                    step.step_id,
                    decision.status,
                    decision.risk_level,
                    decision.policy
                ));
                if decision.requires_approval || decision.status == "denied" {
                    approval_context = Some(ApprovalContext {
                        approval_id: Some(approval_id.clone()),
                        action: action.to_string(),
                        message: decision.message.clone(),
                        risk_level: decision.risk_level.clone(),
                        policy: decision.policy.clone(),
                    });
                }
                if decision.status == "denied" {
                    logs.push("Execution blocked by policy".to_string());
                    blocked = true;
                    blocked_reason = "approval_policy_blocked".to_string();
                    break;
                }
                if decision.requires_approval {
                    approval_required = true;
                    logs.push("Approval required before continuing".to_string());
                } else {
                    logs.push("Approval auto-granted".to_string());
                }
            }
            StepType::Extract => {
                if let Some(summary) = try_extract_summary(plan) {
                    logs.push(format!("Summary: {}", summary));
                } else {
                    logs.push("No summary extracted".to_string());
                }
            }
            StepType::Screenshot => {}
        }

        if manual_required || approval_required || blocked {
            resume_from = Some(idx + 1);
            break;
        }
    }

    if blocked {
        logs.push(format!(
            "Execution stopped with blocked status (reason={})",
            blocked_reason
        ));
        push_focus_handoff_summary(&mut logs, &focus_handoff_state);
        push_run_attempt(&mut logs, "execution_end", "blocked", &blocked_reason);
        let resume_token = build_resume_token(plan, resume_from, &blocked_reason);
        if let Some(token) = resume_token.as_ref() {
            logs.push(format!("RESUME_TOKEN|status=blocked|token={}", token));
        }
        return ExecutionResult {
            status: "blocked".to_string(),
            logs,
            approval: approval_context,
            manual_steps,
            resume_from,
            resume_token,
        };
    }
    if approval_required {
        logs.push("Execution paused awaiting approval".to_string());
        push_focus_handoff_summary(&mut logs, &focus_handoff_state);
        push_run_attempt(
            &mut logs,
            "execution_end",
            "approval_required",
            "awaiting_approval",
        );
        let resume_token = build_resume_token(plan, resume_from, "approval_required");
        if let Some(token) = resume_token.as_ref() {
            logs.push(format!(
                "RESUME_TOKEN|status=approval_required|token={}",
                token
            ));
        }
        return ExecutionResult {
            status: "approval_required".to_string(),
            logs,
            approval: approval_context,
            manual_steps,
            resume_from,
            resume_token,
        };
    }
    if manual_required {
        logs.push("Execution paused for manual input".to_string());
        push_focus_handoff_summary(&mut logs, &focus_handoff_state);
        push_run_attempt(
            &mut logs,
            "execution_end",
            "manual_required",
            "awaiting_manual_input",
        );
        let resume_token = build_resume_token(plan, resume_from, "manual_required");
        if let Some(token) = resume_token.as_ref() {
            logs.push(format!(
                "RESUME_TOKEN|status=manual_required|token={}",
                token
            ));
        }
        return ExecutionResult {
            status: "manual_required".to_string(),
            logs,
            approval: approval_context,
            manual_steps,
            resume_from,
            resume_token,
        };
    }

    push_focus_handoff_summary(&mut logs, &focus_handoff_state);
    push_run_attempt(
        &mut logs,
        "execution_end",
        "completed",
        "all_steps_completed",
    );
    ExecutionResult {
        status: "completed".to_string(),
        logs,
        approval: approval_context,
        manual_steps,
        resume_from,
        resume_token: None,
    }
}
