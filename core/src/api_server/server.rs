use axum::{
    http::{HeaderValue, Method, StatusCode},
    Router,
};
use tower_http::cors::{Any, CorsLayer};

use super::routes::{
    build_agent_routes, build_base_routes, build_control_routes, build_diagnostics_routes,
    build_memory_routes, build_operational_routes, build_recommendation_routes,
};
use super::runtime::{
    mark_api_server_started_at, spawn_workflow_provision_recovery_loop, telegram_polling_requested,
    try_spawn_telegram_listener, TelegramListenerStartOutcome,
};
use super::AppState;
use crate::{db, llm_gateway};

pub fn build_api_router(state: AppState) -> Router {
    Router::new()
        .merge(build_base_routes())
        .merge(build_recommendation_routes())
        .merge(build_control_routes())
        .merge(build_diagnostics_routes())
        .merge(build_operational_routes())
        .merge(build_memory_routes())
        .merge(build_agent_routes())
        .layer(axum::middleware::from_fn(auth_middleware))
        .layer(build_api_cors_layer())
        .with_state(state)
}

pub async fn start_api_server(
    llm_client: Option<std::sync::Arc<dyn llm_gateway::LLMClient>>,
) -> anyhow::Result<()> {
    mark_api_server_started_at();

    let telegram_polling_explicit = std::env::var("STEER_TELEGRAM_POLLING").ok();
    if telegram_polling_requested() {
        if let Some(llm) = llm_client.clone() {
            match try_spawn_telegram_listener(llm) {
                Ok(TelegramListenerStartOutcome::Started) => {
                    if telegram_polling_explicit.is_some() {
                        println!("🤖 Telegram polling enabled (STEER_TELEGRAM_POLLING=1).");
                    } else {
                        println!(
                            "🤖 Telegram polling auto-enabled (Telegram credentials detected)."
                        );
                    }
                }
                Ok(TelegramListenerStartOutcome::AlreadyRunning) => {
                    println!("ℹ️ Telegram listener already running.");
                }
                Err("missing_telegram_token") => {
                    println!(
                        "⚠️  STEER_TELEGRAM_POLLING=1 but TELEGRAM_BOT_TOKEN is missing; listener not started."
                    );
                }
                Err(_) => {}
            }
        } else {
            println!("⚠️  STEER_TELEGRAM_POLLING=1 but LLM is unavailable; listener not started.");
        }
    }

    match db::mark_orphaned_inflight_task_runs_failed() {
        Ok(recovered) if recovered > 0 => {
            println!(
                "♻️ Recovered {} orphaned in-flight task run(s) from previous core process.",
                recovered
            );
        }
        Ok(_) => {}
        Err(error) => {
            println!("⚠️ Failed to recover orphaned task runs: {}", error);
        }
    }

    let state = AppState {
        llm_client,
        current_goal: std::sync::Arc::new(std::sync::Mutex::new(None)),
    };

    spawn_workflow_provision_recovery_loop(state.llm_client.clone());
    let app = build_api_router(state);

    let port = std::env::var("STEER_API_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5680);
    println!("🌐 Desktop API server running on http://localhost:{}", port);

    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", port))
        .await
        .map_err(|error| anyhow::anyhow!("Failed to bind port {}: {}", port, error))?;

    axum::serve(listener, app)
        .await
        .map_err(|error| anyhow::anyhow!("Server error: {}", error))?;
    Ok(())
}

async fn auth_middleware(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    if req.method() == Method::OPTIONS {
        return Ok(next.run(req).await);
    }

    let api_key = std::env::var("STEER_API_KEY").unwrap_or_default();
    let request_path = req.uri().path().to_string();
    let request_method = req.method().to_string();

    if api_key.is_empty() {
        let allow_no_key = crate::env_flag("STEER_API_ALLOW_NO_KEY");
        if allow_no_key {
            return Ok(next.run(req).await);
        } else {
            crate::diagnostic_events::emit(
                "api.auth.denied",
                serde_json::json!({
                    "reason": "no_key_mode_not_allowed",
                    "path": request_path,
                    "method": request_method
                }),
            );
        }
        return Err(StatusCode::UNAUTHORIZED);
    }

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(|value| value.trim().to_string());
    let x_api_key = req
        .headers()
        .get("X-API-Key")
        .and_then(|header| header.to_str().ok())
        .map(|value| value.trim().to_string());

    match auth_header.or(x_api_key) {
        Some(key) if key == api_key => Ok(next.run(req).await),
        _ => {
            crate::diagnostic_events::emit(
                "api.auth.denied",
                serde_json::json!({
                    "reason": "api_key_mismatch_or_missing",
                    "path": request_path,
                    "method": request_method
                }),
            );
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

fn build_api_cors_layer() -> CorsLayer {
    let allowed_origins = [
        "http://localhost:5173"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://localhost:5174"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://localhost:5680"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "tauri://localhost"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://127.0.0.1:5173"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
        "http://127.0.0.1:5174"
            .parse::<HeaderValue>()
            .expect("Invalid CORS origin"),
    ];

    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods(Any)
        .allow_headers(Any)
}
