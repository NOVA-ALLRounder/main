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
    if let Some(llm) = state.llm_client {
        if let Ok(mut guard) = state.current_goal.lock() {
            *guard = Some(payload.goal.clone());
        }
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
            "message": "Autonomous Agent started. Monitor logs for progress."
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

    let run_id = format!(
        "surf_{}_{}",
        chrono::Utc::now().format("%Y%m%d_%H%M%S"),
        uuid::Uuid::new_v4().simple()
    );
    let _ = db::mark_stale_running_task_runs_finished();
    let allow_goal_queue = env_truthy_default("STEER_ALLOW_GOAL_QUEUE", false);
    if !allow_goal_queue {
        match db::claim_singleton_task_run(&run_id, "surf_goal", goal, "queued") {
            Ok(true) => {}
            Ok(false) => {
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

                return (
                    StatusCode::CONFLICT,
                    Json(json!({ "error": "goal_run_already_inflight" })),
                )
                    .into_response();
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "task_run_persist_failed",
                        "detail": error.to_string()
                    })),
                )
                    .into_response();
            }
        }
    } else if let Err(error) = db::create_task_run(&run_id, "surf_goal", goal, "queued") {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "task_run_persist_failed",
                "detail": error.to_string()
            })),
        )
            .into_response();
    }

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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use axum::{body::to_bytes, response::Response};
    use serde_json::Value;
    use serial_test::serial;
    use std::sync::{Arc, Mutex};

    use crate::llm_gateway::{FeedbackAnalysis, LLMClient};
    use crate::recommendation::AutomationProposal;

    struct DummyLlmClient;

    #[async_trait]
    impl LLMClient for DummyLlmClient {
        async fn plan_next_step(
            &self,
            _goal: &str,
            _ui_tree: &Value,
            _action_history: &[String],
        ) -> Result<Value> {
            Err(anyhow!("dummy_llm"))
        }

        async fn chat_completion(&self, _messages: Vec<Value>) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn plan_vision_step(
            &self,
            _goal: &str,
            _image_b64: &str,
            _history: &[String],
        ) -> Result<Value> {
            Err(anyhow!("dummy_llm"))
        }

        async fn analyze_routine(&self, _logs: &[String]) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn recommend_automation(&self, _logs: &[String]) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn build_n8n_workflow(&self, _user_prompt: &str) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn fix_n8n_workflow(
            &self,
            _user_prompt: &str,
            _bad_json: &str,
            _error_msg: &str,
        ) -> Result<String, Box<dyn std::error::Error>> {
            Err(Box::new(std::io::Error::other("dummy_llm")))
        }

        async fn get_embedding(&self, _text: &str) -> Result<Vec<f32>> {
            Err(anyhow!("dummy_llm"))
        }

        async fn propose_workflow(
            &self,
            _logs: &[String],
        ) -> Result<AutomationProposal, Box<dyn std::error::Error>> {
            Err(Box::new(std::io::Error::other("dummy_llm")))
        }

        async fn analyze_tendency(&self, _logs: &[String]) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn parse_intent(&self, _user_input: &str) -> Result<Value> {
            Err(anyhow!("dummy_llm"))
        }

        async fn parse_intent_with_history(
            &self,
            _user_input: &str,
            _history: &[crate::db::ChatMessage],
        ) -> Result<Value> {
            Err(anyhow!("dummy_llm"))
        }

        async fn generate_recommendation_from_pattern(
            &self,
            _pattern_description: &str,
            _sample_events: &[String],
        ) -> Result<AutomationProposal> {
            Err(anyhow!("dummy_llm"))
        }

        async fn analyze_screen(
            &self,
            _prompt: &str,
            _image_b64: &str,
        ) -> Result<String, Box<dyn std::error::Error>> {
            Err(Box::new(std::io::Error::other("dummy_llm")))
        }

        async fn find_element_coordinates(
            &self,
            _element_description: &str,
            _image_b64: &str,
        ) -> Result<Option<(i32, i32)>> {
            Err(anyhow!("dummy_llm"))
        }

        async fn score_quality(
            &self,
            _system_prompt: &str,
            _payload: &serde_json::Value,
        ) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        async fn propose_solution_stack(&self, _goal: &str) -> Result<Value> {
            Err(anyhow!("dummy_llm"))
        }

        async fn inference_local(&self, _prompt: &str, _model: Option<&str>) -> Result<String> {
            Err(anyhow!("dummy_llm"))
        }

        fn route_task(&self, _task_description: &str, _pii_detected: bool) -> (bool, String) {
            (false, "dummy_llm".to_string())
        }

        async fn analyze_user_feedback(
            &self,
            _feedback: &str,
            _history_summary: &str,
        ) -> Result<FeedbackAnalysis> {
            Err(anyhow!("dummy_llm"))
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self { key, previous }
        }

        fn remove(key: &'static str) -> Self {
            let previous = std::env::var(key).ok();
            std::env::remove_var(key);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => std::env::set_var(self.key, value),
                None => std::env::remove_var(self.key),
            }
        }
    }

    struct TestDbGuard {
        _db_path: EnvVarGuard,
    }

    impl TestDbGuard {
        fn new(test_name: &str) -> Self {
            crate::db::reset_connection();
            let db_path = std::env::temp_dir().join(format!(
                "steer_goal_handler_{}_{}.db",
                test_name,
                uuid::Uuid::new_v4()
            ));
            let db_path = EnvVarGuard::set("STEER_DB_PATH", &db_path.to_string_lossy());
            crate::db::init().expect("init test db");
            Self { _db_path: db_path }
        }
    }

    impl Drop for TestDbGuard {
        fn drop(&mut self) {
            crate::db::reset_connection();
        }
    }

    fn test_state_with_dummy_llm() -> AppState {
        AppState {
            llm_client: Some(Arc::new(DummyLlmClient)),
            current_goal: Arc::new(Mutex::new(None)),
        }
    }

    async fn response_json(response: Response) -> Value {
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        serde_json::from_slice(&body).expect("json response body")
    }

    async fn run_goal_response(state: AppState, goal: &str) -> (StatusCode, Value) {
        let response = run_goal_sync_handler(
            State(state),
            Json(RunGoalRequest {
                goal: goal.to_string(),
                session_key: None,
            }),
        )
        .await
        .into_response();
        let status = response.status();
        let payload = response_json(response).await;
        (status, payload)
    }

    #[tokio::test]
    async fn execute_goal_handler_does_not_mutate_current_goal_when_llm_missing() {
        let state = AppState {
            llm_client: None,
            current_goal: Arc::new(Mutex::new(None)),
        };

        let Json(payload) = execute_goal_handler(
            State(state.clone()),
            Json(GoalRequest {
                goal: "ship release".to_string(),
            }),
        )
        .await;

        assert_eq!(payload["status"], "error");
        assert!(state
            .current_goal
            .lock()
            .ok()
            .and_then(|goal| goal.clone())
            .is_none());
    }

    #[tokio::test]
    #[serial]
    async fn run_goal_sync_handler_accepts_and_persists_new_run() {
        let _db = TestDbGuard::new("accepted");
        let _allow_goal_queue = EnvVarGuard::set("STEER_ALLOW_GOAL_QUEUE", "0");
        let _wait_for_result = EnvVarGuard::remove("STEER_GOAL_SYNC_WAIT_FOR_RESULT");
        let state = test_state_with_dummy_llm();

        let (status, payload) = run_goal_response(state, "ship release").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(payload["status"], "accepted");
        let run_id = payload["run_id"].as_str().expect("accepted run id");
        let task_run = crate::db::get_task_run(run_id)
            .expect("load task run")
            .expect("persisted task run");
        assert_eq!(task_run.run_id, run_id);
        assert_eq!(task_run.intent, "surf_goal");
        assert_eq!(task_run.prompt, "ship release");
    }

    #[tokio::test]
    #[serial]
    async fn run_goal_sync_handler_returns_busy_when_inflight_exists() {
        let _db = TestDbGuard::new("busy");
        let _allow_goal_queue = EnvVarGuard::set("STEER_ALLOW_GOAL_QUEUE", "0");
        let _wait_for_result = EnvVarGuard::remove("STEER_GOAL_SYNC_WAIT_FOR_RESULT");
        let active_run_id = format!("active-{}", uuid::Uuid::new_v4());
        crate::db::create_task_run(&active_run_id, "surf_goal", "existing goal", "running")
            .expect("seed inflight task run");

        let (status, payload) = run_goal_response(test_state_with_dummy_llm(), "new goal").await;

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(payload["status"], "busy");
        assert_eq!(payload["run_id"], active_run_id);
        assert!(payload["summary"]
            .as_str()
            .unwrap_or_default()
            .contains("existing run in progress"));
    }

    #[tokio::test]
    #[serial]
    async fn run_goal_sync_handler_returns_persist_failure_when_db_unavailable() {
        let _db = TestDbGuard::new("persist_failed");
        let _allow_goal_queue = EnvVarGuard::set("STEER_ALLOW_GOAL_QUEUE", "0");
        let _wait_for_result = EnvVarGuard::remove("STEER_GOAL_SYNC_WAIT_FOR_RESULT");
        crate::db::reset_connection();

        let (status, payload) = run_goal_response(test_state_with_dummy_llm(), "persist me").await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(payload["error"], "task_run_persist_failed");
        assert!(payload["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("db_unavailable"));
    }
}
