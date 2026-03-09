use axum::{
    routing::{get, post},
    Router,
};

use super::super::automation::analyze_patterns;
use super::super::diagnostics::{
    get_quality_metrics, latest_quality_handler, lock_metrics_handler,
    run_consistency_verification_handler, run_exec_results_guard_handler, run_judgment_handler,
    run_performance_verification_handler, run_release_gate_handler,
    run_runtime_verification_handler, run_semantic_verification_handler,
    run_visual_verification_handler, runtime_db_paths_handler, runtime_info_handler,
    scan_project_handler, score_quality_handler,
};
use super::super::AppState;

pub(crate) fn build_diagnostics_routes() -> Router<AppState> {
    Router::new()
        .route("/api/project/scan", get(scan_project_handler))
        .route(
            "/api/verify/runtime",
            post(run_runtime_verification_handler),
        )
        .route("/api/verify/visual", post(run_visual_verification_handler))
        .route(
            "/api/verify/semantic",
            post(run_semantic_verification_handler),
        )
        .route(
            "/api/verify/performance",
            post(run_performance_verification_handler),
        )
        .route(
            "/api/verify/consistency",
            post(run_consistency_verification_handler),
        )
        .route("/api/judgment", post(run_judgment_handler))
        .route("/api/release/gate", post(run_release_gate_handler))
        .route(
            "/api/exec-results/guard",
            post(run_exec_results_guard_handler),
        )
        .route("/api/quality/score", post(score_quality_handler))
        .route("/api/quality/latest", get(latest_quality_handler))
        .route("/api/patterns/analyze", post(analyze_patterns))
        .route("/api/quality", get(get_quality_metrics))
        .route("/api/system/db-paths", get(runtime_db_paths_handler))
        .route("/api/system/runtime-info", get(runtime_info_handler))
        .route("/api/system/lock-metrics", get(lock_metrics_handler))
}
