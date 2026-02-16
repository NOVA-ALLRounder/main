fn jarvis_chat_request(message: String) -> ChatRequest {
    ChatRequest {
        message,
        channel: Some("jarvis".to_string()),
        chat_type: Some("direct".to_string()),
        sender: Some("web".to_string()),
        mentioned: Some(true),
    }
}

fn jarvis_skill_message(skill_name: &str, action: &str, params: &Option<serde_json::Value>) -> String {
    if let Some(text) = params
        .as_ref()
        .and_then(|p| p.get("text"))
        .and_then(|t| t.as_str())
    {
        return text.to_string();
    }

    match (skill_name, action) {
        ("pattern_analysis", _) => "analyze_patterns".to_string(),
        ("screen_summary", _) => "summarize current screen and put it in notepad".to_string(),
        ("email_workflow", _) => "email workflow template run".to_string(),
        ("calendar_overview", "today") => "오늘 일정 보여줘".to_string(),
        ("calendar_overview", "week") => "이번주 일정 보여줘".to_string(),
        _ => format!("{} {}", skill_name.replace('_', " "), action),
    }
}

pub(super) async fn handle_jarvis_command(
    State(state): State<AppState>,
    Json(req): Json<JarvisCommandRequest>,
) -> Json<serde_json::Value> {
    let chat = handle_chat(State(state), Json(jarvis_chat_request(req.text))).await.0;
    Json(serde_json::json!({
        "success": true,
        "message": chat.response,
        "data": {
            "command": chat.command
        }
    }))
}

pub(super) async fn handle_jarvis_web_message(
    State(state): State<AppState>,
    Json(req): Json<JarvisWebMessageRequest>,
) -> Json<serde_json::Value> {
    let chat = handle_chat(State(state), Json(jarvis_chat_request(req.text))).await.0;
    Json(serde_json::json!({
        "success": true,
        "message": chat.response,
        "data": {
            "command": chat.command,
            "session_key": req.session_key,
            "metadata": req.metadata
        }
    }))
}

pub(super) async fn list_jarvis_skills() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "skills": [
            {
                "name": "pattern_analysis",
                "description": "Analyze recent activity patterns and suggest workflows.",
                "version": "1.0",
                "actions": ["run"],
                "eligible": true,
                "platform": "desktop",
                "tags": ["automation", "analysis"]
            },
            {
                "name": "email_workflow",
                "description": "Run email triage workflow and generate CSV artifacts.",
                "version": "1.0",
                "actions": ["run"],
                "eligible": true,
                "platform": "desktop",
                "tags": ["email", "workflow"]
            },
            {
                "name": "screen_summary",
                "description": "Summarize current screen and deliver results to notepad.",
                "version": "1.0",
                "actions": ["run"],
                "eligible": true,
                "platform": "desktop",
                "tags": ["vision", "productivity"]
            },
            {
                "name": "calendar_overview",
                "description": "Read today's or this week's calendar events.",
                "version": "1.0",
                "actions": ["today", "week"],
                "eligible": true,
                "platform": "desktop",
                "tags": ["calendar"]
            }
        ]
    }))
}

pub(super) async fn execute_jarvis_skill(
    State(state): State<AppState>,
    Path(skill_name): Path<String>,
    Json(req): Json<JarvisSkillExecuteRequest>,
) -> Json<serde_json::Value> {
    let message = jarvis_skill_message(&skill_name, &req.action, &req.params);
    let chat = handle_chat(State(state), Json(jarvis_chat_request(message))).await.0;
    Json(serde_json::json!({
        "success": true,
        "message": chat.response,
        "data": {
            "skill": skill_name,
            "action": req.action,
            "command": chat.command
        }
    }))
}

pub(super) async fn list_jarvis_sessions() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "sessions": []
    }))
}

pub(super) async fn start_jarvis_autonomous() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "success": false,
        "message": "Autonomous mode is not wired in this API build. Use /api/agent/goal.",
        "data": {
            "hint": "POST /api/agent/goal"
        }
    }))
}

pub(super) async fn list_routines() -> Json<Vec<crate::db::Routine>> {
    match crate::db::get_all_routines() {
        Ok(routines) => Json(routines),
        Err(e) => {
            eprintln!("Failed to list routines: {}", e);
            Json(Vec::new())
        }
    }
}

pub(super) async fn create_routine_handler(Json(payload): Json<CreateRoutineRequest>) -> Json<serde_json::Value> {
    match crate::db::create_routine(&payload.name, &payload.cron, &payload.prompt) {
        Ok(id) => Json(serde_json::json!({ "status": "ok", "id": id })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

// --- Issue #2 Fix: Toggle Routine ---
pub(super) async fn toggle_routine_handler(
    axum::extract::Path(id): axum::extract::Path<i64>,
    Json(payload): Json<ToggleRoutineRequest>,
) -> Json<serde_json::Value> {
    match crate::db::toggle_routine(id, payload.enabled) {
        Ok(_) => Json(serde_json::json!({ "status": "ok" })),
        Err(e) => Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
    }
}

pub(super) async fn list_recommendations(
    Query(params): Query<RecQueryParams>,
) -> Json<Vec<RecommendationItem>> {
    // Treat empty string as None; default to "all" for history view.
    let filter = params
        .status
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(Some("all"));

    match db::get_recommendations_with_filter(filter) {
        Ok(recs) => Json(
            recs.into_iter()
                .map(|r| RecommendationItem {
                    id: r.id,
                    status: r.status,
                    title: r.title,
                    summary: r.summary,
                    confidence: r.confidence,
                    evidence: r.evidence, // [NEW] Pass evidence
                    last_error: r.last_error,
                })
                .collect()
        ),
        Err(_) => Json(vec![]),
    }
}

pub(super) async fn approve_recommendation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("???Received approval request for Recommendation ID: {}", id);

    // 1. Get recommendation from DB
    let rec = match db::get_recommendation(id) {
        Ok(Some(r)) => r,
        _ => {
            eprintln!("??Recommendation #{} not found in DB", id);
            return Err((StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Recommendation not found"}))));
        }
    };

    let n8n_client = match n8n_api::N8nApi::from_env() {
        Ok(c) => c,
        Err(e) => {
             return Err((
                 StatusCode::INTERNAL_SERVER_ERROR, 
                 Json(serde_json::json!({ "error": "n8n Client Init Failed", "details": e.to_string() }))
             ));
        }
    };
    
    // Ensure n8n is running first
    if let Err(e) = n8n_client.ensure_server_running().await {
        eprintln!("??Failed to start n8n: {}", e);
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "n8n Server Unavailable", "details": e.to_string() }))
        ));
    }

    // [NEW] Fetch Credentials to inform LLM
    let credentials = n8n_client.list_credentials().await.unwrap_or_default();
    let cred_context = if credentials.is_empty() {
        "NOTE: No credentials found in n8n. Do NOT use nodes requiring authentication (like Gmail, Slack) unless you are sure.".to_string()
    } else {
        let list = credentials.iter()
            .map(|c| format!("- Name: '{}', ID: '{}', Type: '{}'", c.name, c.id, c.type_name))
            .collect::<Vec<_>>()
            .join("\n");
        format!("IMPORTANT: You MUST use these exact Credential IDs for authentication:\n{}\nIf a required credential is missing, do not hallucinate an ID. Use a placeholder and add a comment.", list)
    };

    let mut current_json = if let Some(json) = &rec.workflow_json {
        json.clone()
    } else {
        // Initial Generation with Credential Context
        if let Some(llm) = &state.llm_client {
             let full_prompt = format!("{}\n\n{}", rec.n8n_prompt, cred_context);
             println!("?�?Generating workflow with context: {} credentials", credentials.len());
             
             match llm.build_n8n_workflow(&full_prompt).await {
                Ok(json) => json,
                Err(e) => return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": "LLM Generation Failed", "details": e.to_string() }))
                )),
             }
        } else {
             return Err((
                 StatusCode::SERVICE_UNAVAILABLE,
                 Json(serde_json::json!({ "error": "LLM Client Unavailable" }))
             ));
        }
    };

    let mut attempts = 0;
    let max_attempts = 3;
    let mut last_error = String::new();

    while attempts < max_attempts {
        attempts += 1;
        
        // Repair JSON if this is a retry
        if attempts > 1 {
            if let Some(llm) = &state.llm_client {
                println!("???Attempting to fix workflow JSON (Try {}/{})", attempts, max_attempts);
                // Also pass credential context during fix
                let fix_prompt = format!("{}\n\n{}", rec.n8n_prompt, cred_context);
                match llm.fix_n8n_workflow(&fix_prompt, &current_json, &last_error).await {
                    Ok(fixed) => current_json = fixed,
                    Err(e) => println!("Failed to fix JSON: {}", e),
                }
            }
        }

        // Parse JSON to Value
        let workflow_data: serde_json::Value = match serde_json::from_str(&current_json)
            .or_else(|_| {
                let cleaned = extract_json_object(&current_json);
                serde_json::from_str(&cleaned)
            }) {
            Ok(v) => v,
            Err(e) => {
                last_error = format!("Invalid JSON Syntax: {}", e);
                continue; 
            }
        };

        // Extract name
        let name = workflow_data["name"].as_str().unwrap_or(&rec.title).to_string();
        
        // Try create
        // SAFETY: Created as inactive (false) to prevent broken loops. User must enable manually.
        match n8n_client.create_workflow(&name, &workflow_data, false).await {
            Ok(workflow_id) => {
                // Success!
                println!("??Workflow created successfully on attempt {}", attempts);
                if let Err(e) = db::mark_recommendation_approved(id, &workflow_id, &current_json) {
                    eprintln!("Failed to update DB: {}", e);
                }
                return Ok(Json(serde_json::json!({
                    "status": "success",
                    "id": workflow_id,
                    "message": "Workflow created successfully"
                })));
            },
            Err(e) => {
                last_error = e.to_string();
                println!("??Creation failed: {}", last_error);
            }
        }
    }
    
    // If we get here, all attempts failed
    let error_msg = format!("??All {} attempts to create workflow failed. Last Error: {}", max_attempts, last_error);
    eprintln!("{}", error_msg);
    if let Err(db_err) = db::mark_recommendation_failed(id, &last_error) {
        eprintln!("Failed to mark recommendation as failed: {}", db_err);
    }
    
    Err((
        StatusCode::BAD_GATEWAY,
        Json(serde_json::json!({ "error": error_msg, "details": last_error }))
    ))
}

pub(super) fn extract_json_object(input: &str) -> String {
    let start = input.find('{');
    let end = input.rfind('}');
    match (start, end) {
        (Some(s), Some(e)) if e > s => input[s..=e].to_string(),
        _ => input.to_string(),
    }
}

pub(super) async fn reject_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "rejected") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn later_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "later") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn restore_recommendation(
    axum::extract::Path(id): axum::extract::Path<i64>,
) -> StatusCode {
    match db::update_recommendation_status(id, "pending") {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn list_exec_approvals(
    Query(query): Query<ExecApprovalQuery>,
) -> Json<Vec<db::ExecApproval>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let status = match query.status.as_deref() {
        Some("all") => None,
        other => other,
    };
    let approvals = db::list_exec_approvals(status, limit).unwrap_or_default();
    Json(approvals)
}

pub(super) async fn approve_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    let decision = payload
        .as_ref()
        .and_then(|p| p.decision.as_deref())
        .unwrap_or("allow-once");

    if decision == "allow-always" {
        if let Ok(Some(approval)) = db::get_exec_approval(&id) {
            let _ = db::add_exec_allowlist(&approval.command, approval.cwd.as_deref());
        }
    }

    match db::resolve_exec_approval(&id, "approved", resolved_by, Some(decision)) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn reject_exec_approval(
    Path(id): Path<String>,
    payload: Option<Json<ExecApprovalResolve>>,
) -> StatusCode {
    let resolved_by = payload.as_ref().and_then(|p| p.resolved_by.as_deref());
    match db::resolve_exec_approval(&id, "rejected", resolved_by, Some("deny")) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn list_routine_runs(
    Query(query): Query<RoutineRunsQuery>,
) -> Json<Vec<db::RoutineRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_routine_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(super) async fn list_exec_allowlist(
    Query(query): Query<ExecAllowlistQuery>,
) -> Json<Vec<db::ExecAllowlistEntry>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let entries = db::list_exec_allowlist(limit).unwrap_or_default();
    Json(entries)
}

pub(super) async fn list_exec_results(
    Query(query): Query<ExecResultsQuery>,
) -> Json<Vec<db::ExecResult>> {
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let status = query.status.as_deref();
    let results = db::list_exec_results(status, limit).unwrap_or_default();
    Json(results)
}

pub(super) async fn list_verification_runs(
    Query(query): Query<VerificationRunsQuery>,
) -> Json<Vec<db::VerificationRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_verification_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(super) async fn list_nl_runs_handler(
    Query(query): Query<NLRunQuery>,
) -> Json<Vec<db::NLRun>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let runs = db::list_nl_runs(limit).unwrap_or_default();
    Json(runs)
}

pub(super) async fn nl_run_metrics_handler(
    Query(query): Query<NLRunMetricsQuery>,
) -> Json<db::NLRunMetrics> {
    let limit = query.limit.unwrap_or(50).clamp(1, 500);
    let metrics = db::get_nl_run_metrics(limit).unwrap_or(db::NLRunMetrics {
        total: 0,
        completed: 0,
        manual_required: 0,
        approval_required: 0,
        blocked: 0,
        error: 0,
        success_rate: 0.0,
    });
    Json(metrics)
}

pub(super) async fn list_nl_approval_policies(
    Query(query): Query<ApprovalPolicyQuery>,
) -> Json<Vec<ApprovalPolicyResponse>> {
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let policies = db::list_approval_policies(limit).unwrap_or_default();
    let mapped = policies
        .into_iter()
        .map(|policy| ApprovalPolicyResponse {
            policy_key: policy.policy_key,
            decision: policy.decision,
            updated_at: policy.updated_at,
        })
        .collect();
    Json(mapped)
}

pub(super) async fn set_nl_approval_policy(
    Json(payload): Json<ApprovalPolicyRequest>,
) -> StatusCode {
    if payload.policy_key.trim().is_empty() || payload.decision.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::upsert_approval_policy(&payload.policy_key, &payload.decision) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn remove_nl_approval_policy(
    Path(key): Path<String>,
) -> StatusCode {
    if key.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::delete_approval_policy(&key) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) fn log_verification_run(kind: &str, ok: bool, summary: &str, details: Option<serde_json::Value>) {
    let details_str = details.map(|v| v.to_string());
    let _ = db::insert_verification_run(kind, ok, summary, details_str.as_deref());
}

pub(super) async fn add_exec_allowlist(
    Json(payload): Json<ExecAllowlistRequest>,
) -> StatusCode {
    if payload.pattern.trim().is_empty() {
        return StatusCode::BAD_REQUEST;
    }
    match db::add_exec_allowlist(&payload.pattern, payload.cwd.as_deref()) {
        Ok(_) => StatusCode::CREATED,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn remove_exec_allowlist(
    Path(id): Path<i64>,
) -> StatusCode {
    match db::remove_exec_allowlist(id) {
        Ok(_) => StatusCode::OK,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

pub(super) async fn get_recommendation_metrics() -> Json<RecommendationMetricsResponse> {
    let metrics = db::get_recommendation_metrics().unwrap_or(crate::db::RecommendationMetrics {
        total: 0,
        approved: 0,
        rejected: 0,
        failed: 0,
        pending: 0,
        later: 0,
        last_created_at: None,
    });

    let approval_rate = if metrics.total > 0 {
        (metrics.approved as f64 / metrics.total as f64) * 100.0
    } else {
        0.0
    };

    Json(RecommendationMetricsResponse {
        total: metrics.total,
        approved: metrics.approved,
        rejected: metrics.rejected,
        failed: metrics.failed,
        pending: metrics.pending,
        later: metrics.later,
        approval_rate,
        last_created_at: metrics.last_created_at,
    })
}

// Add at top: use crate::recommendation::AutomationProposal; 

pub(super) async fn analyze_patterns() -> Json<Vec<String>> {
    Json(run_analysis_internal())
}

pub(super) fn run_analysis_internal() -> Vec<String> {
    let detector = pattern_detector::PatternDetector::new();
    let matcher = crate::recommendation::TemplateMatcher::new();
    let patterns = detector.analyze();
    
    // 1. Save only high-signal and template-matched recommendations
    let mut seen_pattern_ids = std::collections::HashSet::new();
    for p in &patterns {
        if !detector.should_recommend(p) {
            continue;
        }
        if !seen_pattern_ids.insert(p.pattern_id.clone()) {
            continue;
        }
        let mut proposal = match matcher.match_pattern(p) {
            Some(proposal) => proposal,
            None => continue,
        };
        if proposal.evidence.is_empty() {
            proposal.evidence = vec![
                format!("Pattern: {}", p.description),
                format!("Frequency: Found {} occurrences", p.occurrences),
            ];
        }
        proposal.pattern_id = Some(p.pattern_id.clone());
        if let Err(e) = db::insert_recommendation(&proposal) {
            eprintln!("Failed to save pattern recommendation: {}", e);
        }
    }

    // 2. Fallback: If empty, create a random demo recommendation (For User Experience)
    // DISABLED: Random spam fix
    /*
    if patterns.is_empty() {
        let timestamp = chrono::Utc::now().timestamp() % 1000;
        let proposal = crate::recommendation::AutomationProposal {
            title: format!("Smart Recommendation #{}", timestamp),
            summary: "AI has identified a potential workflow improvement based on recent activity.".to_string(),
            trigger: "System Activity Analysis".to_string(),
            actions: vec!["Log Activity".to_string(), "Send Notification".to_string()],
            n8n_prompt: "Create a workflow that logs system activity and sends a summary notification.".to_string(),
            confidence: 0.85,
        };
        if let Err(e) = db::insert_recommendation(&proposal) {
            eprintln!("??�묓??Failed to save recommendation analysis: {}", e);
        }
        return vec![];
    }
    */
    
    patterns.into_iter()
        .map(|p| format!("{} ({} occurrences)", p.description, p.occurrences))
        .collect()
}

pub(super) async fn get_quality_metrics() -> Json<QualityMetrics> {
    let collector = feedback_collector::FeedbackCollector::new();
    let metrics = collector.get_quality_metrics();
    
    Json(QualityMetrics {
        total: metrics.total_executions,
        success: metrics.successful_executions,
        rate: metrics.success_rate,
    })
}

pub(super) async fn execute_goal_handler(
    State(state): State<AppState>,
    Json(payload): Json<GoalRequest>,
) -> Json<serde_json::Value> {
    if let Ok(mut guard) = state.current_goal.lock() {
        *guard = Some(payload.goal.clone());
    }
    if let Some(llm) = state.llm_client {
        // Spawn background task for OODA loop
        tokio::spawn(async move {
            let executor = crate::executor::AgentExecutor::new(llm);
            match executor.execute_goal(&payload.goal).await {
                Ok(res) => println!("??Goal Execution Success: {}", res),
                Err(e) => println!("??Goal Execution Failed: {}", e),
            }
        });

        Json(serde_json::json!({
            "status": "started",
            "message": "Autonmous Agent started. Monitor logs for progress."
        }))
    } else {
        Json(serde_json::json!({
            "status": "error",
            "message": "LLM Client not available"
        }))
    }
}

pub(super) async fn get_current_goal(
    State(state): State<AppState>,
) -> Json<serde_json::Value> {
    let goal = state
        .current_goal
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    Json(serde_json::json!({ "goal": goal }))
}

