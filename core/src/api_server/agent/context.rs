use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};

use crate::api_server::AppState;

#[derive(Deserialize)]
pub(crate) struct FeedbackRequest {
    pub goal: String,
    pub feedback: String,
    pub history_summary: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct FeedbackResponse {
    pub action: String,
    pub new_goal: Option<String>,
    pub message: String,
}

pub(crate) async fn handle_feedback(
    State(state): State<AppState>,
    Json(req): Json<FeedbackRequest>,
) -> Json<FeedbackResponse> {
    let goal = if req.goal.trim().is_empty() {
        state
            .current_goal
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .unwrap_or_default()
    } else {
        req.goal.clone()
    };
    let history = req
        .history_summary
        .unwrap_or_else(|| format!("Goal: {}", goal));
    if let Some(llm) = &state.llm_client {
        match llm.analyze_user_feedback(&req.feedback, &history).await {
            Ok(analysis) => {
                let action = analysis.action.clone();
                let new_goal = if action == "refine" {
                    analysis.new_goal.clone().or(Some(goal))
                } else {
                    None
                };
                let message = if action == "refine" {
                    "피드백을 반영해 목표를 업데이트했어요. 다시 실행할 수 있어요.".to_string()
                } else {
                    "피드백을 확인했어요. 작업을 완료로 표시합니다.".to_string()
                };
                return Json(FeedbackResponse {
                    action,
                    new_goal,
                    message,
                });
            }
            Err(e) => {
                eprintln!("Feedback analysis failed: {}", e);
            }
        }
    }

    Json(FeedbackResponse {
        action: "complete".to_string(),
        new_goal: None,
        message: "피드백을 확인했어요. 작업을 완료로 표시합니다.".to_string(),
    })
}

pub(crate) async fn get_selection_context() -> Json<serde_json::Value> {
    match crate::platform::current_platform().selected_text() {
        Ok(Some(text)) => Json(serde_json::json!({ "found": true, "text": text })),
        Ok(None) => Json(serde_json::json!({ "found": false, "text": "" })),
        Err(error) => Json(serde_json::json!({
            "found": false,
            "text": "",
            "error": error.to_string()
        })),
    }
}

pub(crate) async fn list_sessions_handler() -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(guard) => {
            if let Some(store) = guard.as_ref() {
                let sessions: Vec<serde_json::Value> = store
                    .list_active()
                    .iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.id,
                            "key": s.key,
                            "goal": s.goal,
                            "status": format!("{:?}", s.status),
                            "created_at": s.created_at.to_rfc3339(),
                            "updated_at": s.updated_at.to_rfc3339(),
                            "steps_count": s.steps.len(),
                            "can_resume": s.can_resume(),
                        })
                    })
                    .collect();

                Json(serde_json::json!({
                    "success": true,
                    "sessions": sessions,
                    "total": sessions.len()
                }))
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

pub(crate) async fn get_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(guard) => {
            if let Some(store) = guard.as_ref() {
                if let Some(session) = store.get(&id) {
                    Json(serde_json::json!({
                        "success": true,
                        "session": {
                            "id": session.id,
                            "key": session.key,
                            "goal": session.goal,
                            "status": format!("{:?}", session.status),
                            "created_at": session.created_at.to_rfc3339(),
                            "updated_at": session.updated_at.to_rfc3339(),
                            "messages": session.messages,
                            "steps": session.steps,
                            "can_resume": session.can_resume(),
                            "resume_point": session.get_resume_point()
                        }
                    }))
                } else {
                    Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

pub(crate) async fn delete_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                match store.delete(&id) {
                    Ok(true) => Json(serde_json::json!({ "success": true, "deleted": id })),
                    Ok(false) => {
                        Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                    }
                    Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}

pub(crate) async fn resume_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();

    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                if let Some(session) = store.get_mut(&id) {
                    if session.can_resume() {
                        let resume_point = session.get_resume_point();
                        let goal = session.goal.clone();
                        session.status = crate::session_store::SessionStatus::Active;
                        session
                            .add_message("system", &format!("Resuming from step {}", resume_point));

                        Json(serde_json::json!({
                            "success": true,
                            "resumed": true,
                            "session_id": id,
                            "goal": goal,
                            "resume_from_step": resume_point,
                            "message": format!("Session resumed from step {}", resume_point)
                        }))
                    } else {
                        Json(serde_json::json!({
                            "success": false,
                            "error": "Session cannot be resumed (status or no steps)"
                        }))
                    }
                } else {
                    Json(serde_json::json!({ "success": false, "error": "Session not found" }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() })),
    }
}
