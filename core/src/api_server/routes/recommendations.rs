use axum::{
    routing::{get, post},
    Router,
};

use super::super::admin::{get_recommendation_metrics, list_recommendation_review_events_handler};
use super::super::recommendations::{
    approve_recommendation, later_recommendation, list_recommendations, reject_recommendation,
    restore_recommendation, submit_recommendation_feedback,
};
use super::super::AppState;

pub(crate) fn build_recommendation_routes() -> Router<AppState> {
    Router::new()
        .route("/api/recommendations", get(list_recommendations))
        .route(
            "/api/recommendations/:id/approve",
            post(approve_recommendation),
        )
        .route(
            "/api/recommendations/:id/reject",
            post(reject_recommendation),
        )
        .route(
            "/api/recommendations/:id/feedback",
            post(submit_recommendation_feedback),
        )
        .route("/api/recommendations/:id/later", post(later_recommendation))
        .route(
            "/api/recommendations/:id/restore",
            post(restore_recommendation),
        )
        .route(
            "/api/recommendations/metrics",
            get(get_recommendation_metrics),
        )
        .route(
            "/api/recommendations/review-events",
            get(list_recommendation_review_events_handler),
        )
}
