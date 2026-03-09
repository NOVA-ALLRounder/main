#[path = "collector_rs/analytics.rs"]
mod analytics;
#[path = "collector_rs/startup.rs"]
mod startup;
#[path = "collector_rs/support.rs"]
mod support;
#[cfg(test)]
#[path = "collector_rs/tests.rs"]
mod tests;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::get,
    routing::post,
    Json, Router,
};
use local_os_agent::{db, privacy::PrivacyGuard};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use analytics::run_analytics_tick;
use startup::run_startup_workflow_generation;
use support::{parse_events, request_is_authorized, resolve_db_path};

#[derive(Default, Debug)]
struct IngestStats {
    received: u64,
    processed: u64,
    dropped: u64,
    failed: u64,
}

#[derive(Clone)]
struct AppState {
    started_at: Instant,
    stats: Arc<Mutex<IngestStats>>,
    guard: Arc<PrivacyGuard>,
    ingest_token: Option<String>,
}

#[derive(Clone, Debug)]
struct AggregationConfig {
    interval_sec: u64,
    raw_retention_days: i64,
    summary_retention_days: i64,
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

#[derive(Serialize)]
struct StatsResponse {
    uptime_sec: u64,
    received: u64,
    processed: u64,
    dropped: u64,
    failed: u64,
}

impl AggregationConfig {
    fn from_env() -> Self {
        let interval_sec = std::env::var("STEER_COLLECTOR_AGG_INTERVAL_SEC")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(300);

        let raw_retention_days = std::env::var("STEER_COLLECTOR_RAW_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(7);

        let summary_retention_days = std::env::var("STEER_COLLECTOR_SUMMARY_RETENTION_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|v| *v >= 1)
            .unwrap_or(30);

        Self {
            interval_sec,
            raw_retention_days,
            summary_retention_days,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    db::init()?;

    let host = std::env::var("STEER_COLLECTOR_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("STEER_COLLECTOR_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(9100);

    let privacy_salt = std::env::var("PRIVACY_SALT").unwrap_or_else(|_| "default_salt".to_string());
    let ingest_token = std::env::var("STEER_COLLECTOR_TOKEN")
        .ok()
        .or_else(|| std::env::var("COLLECTOR_INGEST_TOKEN").ok())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    let require_ingest_token = std::env::var("STEER_COLLECTOR_REQUIRE_TOKEN")
        .ok()
        .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(true);
    if require_ingest_token && ingest_token.is_none() {
        anyhow::bail!(
            "collector_rs token is required. Set STEER_COLLECTOR_TOKEN (or COLLECTOR_INGEST_TOKEN) or set STEER_COLLECTOR_REQUIRE_TOKEN=0 for local-only dev."
        );
    }

    let db_path = resolve_db_path();
    let output_dir = PathBuf::from(
        std::env::var("STEER_WORKFLOW_OUTPUT_DIR").unwrap_or_else(|_| "workflows".to_string()),
    );

    let min_events = std::env::var("STEER_STARTUP_MIN_EVENTS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(100);
    let pattern_threshold = std::env::var("STEER_STARTUP_PATTERN_THRESHOLD")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(3);

    if let Err(err) =
        run_startup_workflow_generation(&db_path, &output_dir, min_events, pattern_threshold)
    {
        eprintln!("⚠️ startup workflow generation failed: {err}");
    }

    let aggregation_config = AggregationConfig::from_env();
    let analytics_db_path = db_path.clone();
    let analytics_cfg = aggregation_config.clone();

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(analytics_cfg.interval_sec));
        loop {
            ticker.tick().await;
            if let Err(err) = run_analytics_tick(&analytics_db_path, &analytics_cfg) {
                eprintln!("⚠️ analytics tick failed: {err}");
            }
        }
    });

    let state = AppState {
        started_at: Instant::now(),
        stats: Arc::new(Mutex::new(IngestStats::default())),
        guard: Arc::new(PrivacyGuard::new(privacy_salt)),
        ingest_token,
    };

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/stats", get(stats_handler))
        .route("/events", post(ingest_events_handler))
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    println!("collector_rs listening on http://{addr}");
    println!("POST /events | GET /health | GET /stats");
    println!(
        "analytics: {}s interval, raw={}d, summary={}d",
        aggregation_config.interval_sec,
        aggregation_config.raw_retention_days,
        aggregation_config.summary_retention_days
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn stats_handler(State(state): State<AppState>) -> Json<StatsResponse> {
    let stats = state.stats.lock().await;
    Json(StatsResponse {
        uptime_sec: state.started_at.elapsed().as_secs(),
        received: stats.received,
        processed: stats.processed,
        dropped: stats.dropped,
        failed: stats.failed,
    })
}

async fn ingest_events_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<Value>,
) -> (StatusCode, Json<Value>) {
    if !request_is_authorized(&state, &headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "status": "error",
                "error": "unauthorized"
            })),
        );
    }

    let events = match parse_events(payload) {
        Ok(events) => events,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "status": "error",
                    "error": msg
                })),
            )
        }
    };

    if events.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "status": "error",
                "error": "no valid events"
            })),
        );
    }

    {
        let mut stats = state.stats.lock().await;
        stats.received += events.len() as u64;
    }

    let mut processed = 0u64;
    let mut dropped = 0u64;
    let mut failed = 0u64;

    for event in events {
        match state.guard.apply(event) {
            Some(masked) => {
                if db::insert_event_v2(&masked).is_ok() {
                    processed += 1;
                } else {
                    failed += 1;
                }
            }
            None => {
                dropped += 1;
            }
        }
    }

    {
        let mut stats = state.stats.lock().await;
        stats.processed += processed;
        stats.dropped += dropped;
        stats.failed += failed;
    }

    let code = if failed > 0 {
        StatusCode::MULTI_STATUS
    } else {
        StatusCode::OK
    };

    (
        code,
        Json(json!({
            "status": "queued",
            "received": processed + dropped + failed,
            "processed": processed,
            "dropped": dropped,
            "failed": failed
        })),
    )
}
