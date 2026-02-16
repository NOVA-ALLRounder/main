use axum::{
    extract::{State, Query, Path},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post},
    response::IntoResponse,
    Json, Router,
};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    approval_gate, chat_sanitize, collector_bridge, consistency_check, context_pruning, db,
    execution_controller, feedback_collector, integrations, intent_router, judgment, llm_gateway,
    monitor, n8n_api, nl_store, pattern_detector, performance_verification, plan_builder,
    project_scanner, quality_scorer, release_gate, runtime_verification, semantic_verification,
    slot_filler, tool_result_guard, verification_engine, visual_verification,
};
use crate::api_dcp::{get_dcp_workflow_status_handler, run_dcp_workflow_handler};
use crate::api_email_workflow::{derive_email_workflow_hints, score_email_priority};
use crate::api_screen_quality::{is_summary_usable, looks_like_steer_overlay_text, normalize_screen_summary, score_summary_output_quality, score_text_signal_quality};
use crate::api_system::{get_recent_logs, get_system_status};
use crate::api_server_types::*;
use crate::api_utils::{
    is_valid_notion_database_id, list_files_human, open_path_windows, open_url_windows,
    write_email_ops_artifacts,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[path = "api_server/handlers.rs"]
mod api_server_handlers;
use api_server_handlers::*;

const PUBLIC_API_PATHS: [&str; 4] = ["/", "/api/health", "/api/version", "/api/system/health"];

fn is_public_api_path(path: &str) -> bool {
    PUBLIC_API_PATHS.contains(&path)
}

fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(auth_value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
    {
        if let Some(token) = auth_value.strip_prefix("Bearer ") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        } else if !auth_value.is_empty() {
            return Some(auth_value.to_string());
        }
    }

    headers
        .get("x-api-key")
        .and_then(|h| h.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn timing_safe_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut diff: u8 = 0;
    for (lhs, rhs) in a.as_bytes().iter().zip(b.as_bytes().iter()) {
        diff |= lhs ^ rhs;
    }
    diff == 0
}

async fn auth_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    let path = req.uri().path().to_string();
    if is_public_api_path(path.as_str()) {
        return Ok(next.run(req).await);
    }

    let configured_key = std::env::var("STEER_API_KEY").unwrap_or_default();
    let require_key = crate::env_flag("STEER_API_REQUIRE_KEY");

    if configured_key.trim().is_empty() {
        if require_key {
            return Err(StatusCode::UNAUTHORIZED);
        }
        return Ok(next.run(req).await);
    }

    let provided_key = extract_api_key(req.headers());
    match provided_key {
        Some(key) if timing_safe_eq(key.trim(), configured_key.trim()) => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

include!("api_server/routes.inc.rs");

/// Start the HTTP API server for desktop GUI
pub async fn start_api_server(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    let state = AppState {
        llm_client,
        current_goal: Arc::new(Mutex::new(None)),
        runtime_control: Arc::new(Mutex::new(crate::runtime_mode::RuntimeControl::from_env())),
    };
    
    // NOTE: Tauri WebView origin can vary by runtime/build. Use permissive local CORS to avoid false failures.
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = build_api_router(state, cors);

    let port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(5680);
    println!("???Desktop API server running on http://localhost:{}", port);
    
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|e| anyhow::anyhow!("Failed to bind port {}: {}", port, e))?;
        
    axum::serve(listener, app).await.map_err(|e| anyhow::anyhow!("Server error: {}", e))?;
    Ok(())
}

async fn root_handler() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "online",
        "service": "Steer OS Core API",
        "version": "v0.1.0",
        "ui_url": "http://localhost:5174",
        "docs": "/api/health"
    }))
}

async fn health_check() -> &'static str {
    "ok"
}

async fn get_version() -> Json<VersionResponse> {
    let build_profile = if cfg!(debug_assertions) { "debug" } else { "release" };
    let git_sha = std::env::var("STEER_GIT_SHA").ok().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    Json(VersionResponse {
        core_version: env!("CARGO_PKG_VERSION").to_string(),
        build_profile: build_profile.to_string(),
        git_sha,
    })
}

async fn get_runtime_mode(
    State(state): State<AppState>,
) -> Json<RuntimeModeResponse> {
    let ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}

async fn set_runtime_mode(
    State(state): State<AppState>,
    Json(payload): Json<SetRuntimeModeRequest>,
) -> Json<RuntimeModeResponse> {
    let mut ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    ctl.mode = crate::runtime_mode::OperationMode::from_str(&payload.mode);
    if let Ok(mut guard) = state.runtime_control.lock() {
        *guard = ctl.clone();
    }

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}

async fn set_emergency_stop(
    State(state): State<AppState>,
    Json(payload): Json<EmergencyStopRequest>,
) -> Json<RuntimeModeResponse> {
    let mut ctl = state
        .runtime_control
        .lock()
        .map(|g| g.clone())
        .unwrap_or_else(|_| crate::runtime_mode::RuntimeControl::from_env());
    ctl.emergency_stop = payload.enabled;
    if let Ok(mut guard) = state.runtime_control.lock() {
        *guard = ctl.clone();
    }

    Json(RuntimeModeResponse {
        mode: ctl.mode.as_str().to_string(),
        emergency_stop: ctl.emergency_stop,
        allow_automation: ctl.allow_automation(),
    })
}


#[path = "api_server/screen.rs"]
mod api_server_screen;
use api_server_screen::*;

#[path = "api_server/system.rs"]
mod api_server_system;
use api_server_system::*;

// Ingest Handler (Replaces Python main.py)
#[path = "api_server/ingest.rs"]
mod api_server_ingest;
use api_server_ingest::*;

#[path = "api_server/chat.rs"]
mod api_server_chat;
use api_server_chat::*;


#[path = "api_server/misc.rs"]
mod api_server_misc;
use api_server_misc::*;

// =====================================================
// SESSION MANAGEMENT HANDLERS (Clawdbot-ported)
// =====================================================




