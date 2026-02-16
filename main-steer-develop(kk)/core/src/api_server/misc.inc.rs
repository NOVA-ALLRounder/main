pub(super) async fn handle_feedback(
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
    let history = req.history_summary.unwrap_or_else(|| format!("Goal: {}", goal));
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
                    "??怨뺢덧?占쎌룄????占쎌룇占???嶺뚮ㅄ維싷쭗?占쎈ご????占쎌몥??袁⑤콦???占쎌꽑?? ???占쎈뻣 ???占쎈뺄???????占쎌꽑??".to_string()
                } else {
                    "??怨뺢덧?占쎌룄????筌먦끉占???占쎌꽑?? ??占쏙옙?????熬곣뫁?占썲슖???占?占쏙옙??紐껊퉵??".to_string()
                };
                return Json(FeedbackResponse { action, new_goal, message });
            }
            Err(e) => {
                eprintln!("Feedback analysis failed: {}", e);
            }
        }
    }

    Json(FeedbackResponse {
        action: "complete".to_string(),
        new_goal: None,
        message: "??怨뺢덧?占쎌룄????筌먦끉占???占쎌꽑?? ??占쏙옙?????熬곣뫁?占썲슖???占?占쏙옙??紐껊퉵??".to_string(),
    })
}

// [Context] Selection Handler
pub(super) async fn get_selection_context() -> Json<serde_json::Value> {
    #[cfg(target_os = "macos")]
    {
        match crate::macos::accessibility::get_selected_text() {
            Some(text) => Json(serde_json::json!({ "found": true, "text": text })),
            None => Json(serde_json::json!({ "found": false, "text": "" })),
        }
    }
    #[cfg(not(target_os = "macos"))]
    Json(serde_json::json!({ "found": false, "text": "", "error": "Not supported on this OS" }))
}

// =====================================================

