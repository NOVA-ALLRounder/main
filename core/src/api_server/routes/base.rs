use axum::{
    routing::{get, post},
    Json, Router,
};

use crate::schema::EventEnvelope;

use super::super::automation::{run_ai_digest_handler, run_news_summary_handler};
use super::super::chat::{handle_chat, handle_chat_feedback};
use super::super::diagnostics::{
    get_recent_logs, get_system_health, get_system_status, health_check, root_handler,
};
use super::super::AppState;

pub(crate) fn build_base_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(root_handler))
        .route("/api/health", get(health_check))
        .route("/api/status", get(get_system_status))
        .route("/api/logs", get(get_recent_logs))
        .route("/api/system/health", get(get_system_health))
        .route("/events", post(ingest_events))
        .route("/api/chat", post(handle_chat))
        .route("/api/chat/feedback", post(handle_chat_feedback))
        .route("/api/automation/ai-digest", post(run_ai_digest_handler))
        .route("/api/llm/news-summary", post(run_news_summary_handler))
}

async fn ingest_events(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    let events: Vec<EventEnvelope> = if let Some(items) = payload.as_array() {
        items
            .iter()
            .filter_map(|value| serde_json::from_value(value.clone()).ok())
            .collect()
    } else if let Ok(single) = serde_json::from_value(payload.clone()) {
        vec![single]
    } else {
        return Json(serde_json::json!({ "error": "Invalid Event Format", "count": 0 }));
    };

    let count = events.len();
    let salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
    let guard = crate::privacy::PrivacyGuard::new(salt);

    let mut success = 0;
    for event in events {
        if let Some(masked_event) = guard.apply(event) {
            if let Err(error) = crate::db::insert_event_v2(&masked_event) {
                eprintln!("Ingest Error: {}", error);
            } else {
                success += 1;
            }
        } else {
            println!("Event dropped by PrivacyGuard");
        }
    }

    Json(serde_json::json!({
        "status": "queued",
        "received": count,
        "processed": success
    }))
}
