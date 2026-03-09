use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

use crate::db;

use super::super::env_truthy_default;
use super::types::{GoalRequest, RunGoalRequest, RunGoalResponse};
use crate::api_server::AppState;

pub(crate) async fn execute_goal_handler(
    State(state): State<AppState>,
    Json(payload): Json<GoalRequest>,
) -> Json<serde_json::Value> {
    if let Ok(mut guard) = state.current_goal.lock() {
        *guard = Some(payload.goal.clone());
    }
    if let Some(llm) = state.llm_client {
        tokio::spawn(async move {
            let planner = crate::controller::planner::Planner::new(llm, None);
            match planner.run_goal_tracked(&payload.goal, None).await {
                Ok(outcome) => println!(
                    "✅ Goal Execution Success (run_id={}, planner={}, execution={}, business={})",
                    outcome.run_id,
                    outcome.planner_complete,
                    outcome.execution_complete,
                    outcome.business_complete
                ),
                Err(e) => println!("❌ Goal Execution Failed: {}", e),
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

pub(crate) async fn run_goal_sync_handler(
    State(state): State<AppState>,
    Json(payload): Json<RunGoalRequest>,
) -> impl IntoResponse {
    let goal = payload.goal.trim();
    if goal.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "goal_empty" })),
        )
            .into_response();
    }

    let Some(llm) = state.llm_client.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "llm_client_not_available" })),
        )
            .into_response();
    };

    let _ = db::mark_stale_running_task_runs_finished();
    let allow_goal_queue = env_truthy_default("STEER_ALLOW_GOAL_QUEUE", false);
    if !allow_goal_queue {
        if let Ok(Some(active)) = db::get_latest_inflight_task_run() {
            return (
                StatusCode::ACCEPTED,
                Json(RunGoalResponse {
                    run_id: active.run_id,
                    planner_complete: false,
                    execution_complete: false,
                    business_complete: false,
                    status: "busy".to_string(),
                    summary: Some(format!(
                        "existing run in progress (status={}). join active run instead of queueing a new one.",
                        active.status
                    )),
                }),
            )
                .into_response();
        }
    }

    let run_id = format!(
        "surf_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        uuid::Uuid::new_v4().simple()
    );
    let _ = db::create_task_run(&run_id, "surf_goal", goal, "queued");
    let spawned_run_id = run_id.clone();
    let goal_owned = goal.to_string();
    let session_owned = payload.session_key.clone();
    let planner = crate::controller::planner::Planner::new(llm, None);
    let (tx, rx) = tokio::sync::oneshot::channel();
    tokio::spawn(async move {
        let result = planner
            .run_goal_tracked_with_run_id(&spawned_run_id, &goal_owned, session_owned.as_deref())
            .await;
        let _ = tx.send(result);
    });

    let wait_for_result = std::env::var("STEER_GOAL_SYNC_WAIT_FOR_RESULT")
        .ok()
        .map(|v| {
            matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false);
    if !wait_for_result {
        return (
            StatusCode::ACCEPTED,
            Json(RunGoalResponse {
                run_id,
                planner_complete: false,
                execution_complete: false,
                business_complete: false,
                status: "accepted".to_string(),
                summary: Some("goal accepted and running asynchronously".to_string()),
            }),
        )
            .into_response();
    }

    let sync_timeout_sec = std::env::var("STEER_GOAL_SYNC_TIMEOUT_SEC")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    match tokio::time::timeout(std::time::Duration::from_secs(sync_timeout_sec), rx).await {
        Ok(Ok(Ok(outcome))) => (
            StatusCode::OK,
            Json(RunGoalResponse {
                run_id: outcome.run_id,
                planner_complete: outcome.planner_complete,
                execution_complete: outcome.execution_complete,
                business_complete: outcome.business_complete,
                status: outcome.status,
                summary: outcome.summary,
            }),
        )
            .into_response(),
        Ok(Ok(Err(e))) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "goal_run_failed", "detail": e.to_string() })),
        )
            .into_response(),
        Ok(Err(_recv_err)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "goal_run_failed", "detail": "planner_channel_closed" })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::ACCEPTED,
            Json(RunGoalResponse {
                run_id,
                planner_complete: false,
                execution_complete: false,
                business_complete: false,
                status: "accepted".to_string(),
                summary: Some(format!(
                    "goal still running asynchronously (sync timeout {}s)",
                    sync_timeout_sec
                )),
            }),
        )
            .into_response(),
    }
}

pub(crate) async fn get_current_goal(State(state): State<AppState>) -> Json<serde_json::Value> {
    let goal = state
        .current_goal
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .unwrap_or_default();
    Json(serde_json::json!({ "goal": goal }))
}
