pub(super) async fn list_sessions_handler() -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();
    
    match crate::session_store::get_session_store() {
        Ok(guard) => {
            if let Some(store) = guard.as_ref() {
                let sessions: Vec<serde_json::Value> = store.list_active()
                    .iter()
                    .map(|s| serde_json::json!({
                        "id": s.id,
                        "key": s.key,
                        "goal": s.goal,
                        "status": format!("{:?}", s.status),
                        "created_at": s.created_at.to_rfc3339(),
                        "updated_at": s.updated_at.to_rfc3339(),
                        "steps_count": s.steps.len(),
                        "can_resume": s.can_resume(),
                    }))
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
    }
}

/// Get specific session by ID
pub(super) async fn get_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
    }
}

/// Delete a session
pub(super) async fn delete_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();
    
    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                match store.delete(&id) {
                    Ok(true) => Json(serde_json::json!({ "success": true, "deleted": id })),
                    Ok(false) => Json(serde_json::json!({ "success": false, "error": "Session not found" })),
                    Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
                }
            } else {
                Json(serde_json::json!({ "success": false, "error": "Store not initialized" }))
            }
        }
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
    }
}

/// Resume a paused/failed session
pub(super) async fn resume_session_handler(Path(id): Path<String>) -> Json<serde_json::Value> {
    let _ = crate::session_store::init_session_store();
    
    match crate::session_store::get_session_store() {
        Ok(mut guard) => {
            if let Some(store) = guard.as_mut() {
                if let Some(session) = store.get_mut(&id) {
                    if session.can_resume() {
                        let resume_point = session.get_resume_point();
                        let goal = session.goal.clone();
                        session.status = crate::session_store::SessionStatus::Active;
                        session.add_message("system", &format!("Resuming from step {}", resume_point));
                        
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
        Err(e) => Json(serde_json::json!({ "success": false, "error": e.to_string() }))
    }
}




