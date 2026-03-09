use axum::Router;

use super::AppState;

mod agent;
mod base;
mod control;
mod diagnostics;
mod memory;
mod operational;
mod recommendations;

pub(super) fn build_base_routes() -> Router<AppState> {
    base::build_base_routes()
}

pub(super) fn build_recommendation_routes() -> Router<AppState> {
    recommendations::build_recommendation_routes()
}

pub(super) fn build_control_routes() -> Router<AppState> {
    control::build_control_routes()
}

pub(super) fn build_diagnostics_routes() -> Router<AppState> {
    diagnostics::build_diagnostics_routes()
}

pub(super) fn build_operational_routes() -> Router<AppState> {
    operational::build_operational_routes()
}

pub(super) fn build_memory_routes() -> Router<AppState> {
    memory::build_memory_routes()
}

pub(super) fn build_agent_routes() -> Router<AppState> {
    agent::build_agent_routes()
}
