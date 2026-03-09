use axum::{
    routing::{get, post},
    Router,
};

use super::super::admin::{
    delete_execution_memory_handler, delete_request_memory_handler, list_memory_records_handler,
    restore_execution_memory_handler, restore_request_memory_handler,
    suppress_execution_memory_handler, suppress_request_memory_handler,
};
use super::super::AppState;

pub(crate) fn build_memory_routes() -> Router<AppState> {
    Router::new()
        .route("/api/memory/records", get(list_memory_records_handler))
        .route(
            "/api/memory/request/suppress",
            post(suppress_request_memory_handler),
        )
        .route(
            "/api/memory/request/restore",
            post(restore_request_memory_handler),
        )
        .route(
            "/api/memory/request/delete",
            post(delete_request_memory_handler),
        )
        .route(
            "/api/memory/execution/suppress",
            post(suppress_execution_memory_handler),
        )
        .route(
            "/api/memory/execution/restore",
            post(restore_execution_memory_handler),
        )
        .route(
            "/api/memory/execution/delete",
            post(delete_execution_memory_handler),
        )
}
