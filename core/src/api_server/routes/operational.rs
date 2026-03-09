use axum::{
    routing::{get, post},
    Router,
};

use super::super::admin::get_launch_ops_handler;
use super::super::operational::{
    get_http_e2e_history_handler, get_latest_http_e2e_handler,
    get_latest_release_readiness_handler, get_launch_eval_candidate_snapshot_info_handler,
    get_launch_eval_candidates_handler, get_release_readiness_history_handler,
    run_http_e2e_handler, run_release_readiness_handler, set_release_baseline_handler,
    write_launch_eval_candidate_snapshot_handler,
};
use super::super::AppState;

pub(crate) fn build_operational_routes() -> Router<AppState> {
    Router::new()
        .route("/api/release/baseline", post(set_release_baseline_handler))
        .route(
            "/api/release/readiness",
            get(get_latest_release_readiness_handler).post(run_release_readiness_handler),
        )
        .route(
            "/api/release/readiness/history",
            get(get_release_readiness_history_handler),
        )
        .route("/api/http-e2e/latest", get(get_latest_http_e2e_handler))
        .route("/api/http-e2e/history", get(get_http_e2e_history_handler))
        .route("/api/http-e2e/run", post(run_http_e2e_handler))
        .route("/api/launch/ops", get(get_launch_ops_handler))
        .route(
            "/api/launch/eval-candidates",
            get(get_launch_eval_candidates_handler),
        )
        .route(
            "/api/launch/eval-candidates/snapshot",
            get(get_launch_eval_candidate_snapshot_info_handler)
                .post(write_launch_eval_candidate_snapshot_handler),
        )
}
